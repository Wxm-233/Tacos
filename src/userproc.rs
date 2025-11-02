//! User process.
//!

mod load;

use alloc::string::String;
use alloc::vec::Vec;
use core::arch::asm;
use core::mem::MaybeUninit;
use core::num::Wrapping;
use core::ops::Sub;
use core::panic;
use riscv::register::sstatus;

use crate::fs::File;
use crate::mem::{PageAlign, PageTable, PhysAddr};
use crate::mem::pagetable::KernelPgTable;
use crate::{childinfo, sbi};
use crate::sync::Semaphore;
use crate::thread::{self, Manager, manager};
use crate::trap::{trap_exit_u, Frame};

use core::convert::TryInto;
use core::ptr::write_bytes;
use ptr::copy_nonoverlapping;
use alloc::sync::Arc;
use crate::mem::Translate;
use crate::childinfo::ChildInfo;
use crate::thread::{schedule};

pub struct UserProc {
    #[allow(dead_code)]
    bin: File,
}

impl UserProc {
    pub fn new(file: File) -> Self {
        Self { bin: file }
    }
}

/// Execute an object file with arguments.
///
/// ## Return
/// - `-1`: On error.
/// - `tid`: Tid of the newly spawned thread.
#[allow(unused_variables)]
pub fn execute(mut file: File, argv: Vec<String>) -> isize {
    #[cfg(feature = "debug")]
    kprintln!(
        "[PROCESS] Kernel thread {} prepare to execute a process with args {:?}",
        thread::current().name(),
        argv
    );

    // It only copies L2 pagetable. This approach allows the new thread
    // to access kernel code and data during syscall without the need to
    // switch pagetables.
    let mut pt = KernelPgTable::clone();

    let (exec_info, stack_va) = match load::load_executable(&mut file, &mut pt) {
        Ok(x) => x,
        Err(_) => unsafe {
            pt.destroy();
            return -1;
        },
    };

    // Initialize frame, pass argument to user.
    let mut frame = unsafe { MaybeUninit::<Frame>::zeroed().assume_init() };
    frame.sepc = exec_info.entry_point;

    // Here the new process will be created.
    let userproc = UserProc::new(file);

    // TODO: (Lab2) Pass arguments to user program

    const LEN_BYTE: usize = core::mem::size_of::<usize>();

    let mut sp = stack_va + 4096;
    let offset = exec_info.init_sp - sp;
    let mut arg_ptrs = Vec::new();
    for arg in argv.iter() {
        let bytes = (arg.as_bytes().len() / LEN_BYTE + 1) * LEN_BYTE;
        sp -= bytes;

        if sp < stack_va {
            return -1;
        }

        unsafe {
            #[cfg(feature = "debug")]
            {
                kprintln!("[STACK] push arg: {} with length {}", arg, arg.len());
            }
            copy_nonoverlapping(arg.as_ptr(), sp as *mut u8, arg.len());
            write_bytes((sp + arg.len()) as *mut u8, 0, bytes - arg.len());
        }
        arg_ptrs.push(sp + offset);
    }

    sp -= LEN_BYTE; // for null pointer
    if sp < stack_va {
        return -1;
    }
    unsafe { write_bytes(sp as *mut u8, 0, 1) }

    sp -= arg_ptrs.len() * LEN_BYTE;
    if sp < stack_va {
        return -1;
    }
    unsafe {
        copy_nonoverlapping(arg_ptrs.as_ptr(), sp as *mut usize, arg_ptrs.len());
    }

    sp -= LEN_BYTE; // for return address
    if sp < stack_va {
        return -1;
    }
    unsafe { write_bytes(sp as *mut u8, 0, 1) }

    let argc = arg_ptrs.len();
    let argv: usize = sp + LEN_BYTE + offset;

    frame.x[2]  = sp + offset; // sp
    frame.x[10] = argc; // a0
    frame.x[11] = argv; // a1

    let child = thread::Builder::new(move || start(frame))
        .pagetable(pt)
        .userproc(userproc)
        .parent(thread::current())
        .spawn();

    let childinfo = child.init_child_info();
    thread::current().children.lock().push(childinfo);
    child.id()
}

/// Exits a process.
///
/// Panic if the current thread doesn't own a user process.
pub fn exit(value: isize) -> ! {
    // TODO: Lab2.
    let old = sbi::interrupt::set(false);
    let current = thread::current();
    if current.userproc().is_none() {
        panic!("exit() called by a non-user thread");
    }

    current.parent.lock().as_ref().map(|parent| {
        parent
            .children
            .lock()
            .iter_mut()
            .find(|child| child.tid == current.id())
            .map(|child_info| {
                child_info.ptr = None;
                child_info.exit_code = Some(value);
                if child_info.is_waiting {
                    child_info.wait_sema.up();
                }
            });
        parent
            .children
            .lock()
            .retain(|child_info| {
                child_info.is_waiting
                || child_info.ptr.is_some()
                || child_info.exit_code.is_some()
        })
    });

    // let current_pt = unsafe { PageTable::effective_pagetable() };
    
    {
        let current = Manager::get().current.lock();
        
        #[cfg(feature = "debug")]
        kprintln!("Exit: {:?}", *current);
        
        current.set_status(thread::imp::Status::Dying);
    } // replace exit() so we can set interrupt here
    sbi::interrupt::set(old);

    schedule();

    unreachable!("An exited thread shouldn't be scheduled again");
}

/// Waits for a child thread, which must own a user process.
///
/// ## Return
/// - `Some(exit_value)`
/// - `None`: if tid was not created by the current thread.
pub fn wait(tid: isize) -> Option<isize> {
    // TODO: Lab2.
    // let old = sbi::interrupt::set(false);
    let current = thread::current();

    // for childinfo in current.children.lock().iter_mut() {
    //     if childinfo.tid == tid {
    //         if let Some(ret) = childinfo.exit_code {
    //             childinfo.exit_code = Some(-1);
    //             return Some(ret);
    //         } 
    //     }
    // }

    let sema = thread::current()
        .children
        .lock()
        .iter_mut()
        .find(|child_info| child_info.tid == tid)
        .map(|child_info| {
            child_info.is_waiting = true;
            child_info.wait_sema.clone()
        });

    // divide into 2 lines for debugging
    sema.map(|sema| {
        sema.down();
    });

    let retval = thread::current()
        .children
        .lock()
        .iter_mut()
        .find(|child_info| child_info.tid == tid)
        .take()
        .map(|child_info| child_info.exit_code.unwrap());

    current.children.lock().retain(|child_info| child_info.tid != tid);

    // sbi::interrupt::set(old);

    retval
}

/// Initializes a user process in current thread.
///
/// This function won't return.
pub fn start(mut frame: Frame) -> ! {
    unsafe { sstatus::set_spp(sstatus::SPP::User) };
    frame.sstatus = sstatus::read();

    // Set kernel stack pointer to intr frame and then jump to `trap_exit_u()`.
    let kernal_sp = (&frame as *const Frame) as usize;

    unsafe {
        asm!(
            "mv sp, t0",
            "jr t1",
            in("t0") kernal_sp,
            in("t1") trap_exit_u as *const u8
        );
    }

    unreachable!();
}
