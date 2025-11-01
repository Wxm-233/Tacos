//! User process.
//!

mod load;

use alloc::string::String;
use alloc::vec::Vec;
use core::arch::asm;
use core::mem::MaybeUninit;
use riscv::register::sstatus;

use crate::fs::File;
use crate::mem::PhysAddr;
use crate::mem::pagetable::KernelPgTable;
use crate::thread::{self, manager};
use crate::trap::{trap_exit_u, Frame};

use core::convert::TryInto;
use core::ptr::write_bytes;
use ptr::copy_nonoverlapping;

use alloc::slice;
use crate::mem::palloc::UserPool;

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

    let exec_info = match load::load_executable(&mut file, &mut pt) {
        Ok(x) => x,
        Err(_) => unsafe {
            pt.destroy();
            return -1;
        },
    };

    // Initialize frame, pass argument to user.
    let mut frame = unsafe { MaybeUninit::<Frame>::zeroed().assume_init() };
    frame.sepc = exec_info.entry_point;
    frame.x[2] = exec_info.init_sp;

    // Here the new process will be created.
    let userproc = UserProc::new(file);

    // TODO: (Lab2) Pass arguments to user program

    let mut sp = PhysAddr::from_pa(exec_info.init_sp).into_va();
    let mut arg_ptrs = Vec::new();
    for arg in argv.iter().rev() {
        sp -= arg.len() + 1;
        sp &= !0xf; // align to 16 bytes
        unsafe {
            #[cfg(feature = "debug")]
            {
                kprintln!("[STACK] push arg: {} with length {}", arg, arg.len());
            }
            copy_nonoverlapping(arg.as_ptr(), sp as *mut u8, arg.len());
            write_bytes((sp + arg.len()) as *mut u8, 0, 1);
        }
        arg_ptrs.push(sp);
    }

    sp -= arg_ptrs.len() * core::mem::size_of::<usize>();
    sp &= !0xf; // align to 16 bytes
    unsafe {
        copy_nonoverlapping(arg_ptrs.as_ptr(), sp as *mut usize, arg_ptrs.len());
    }

    let argc = arg_ptrs.len();
    let argv: usize = sp;

    // frame.x[2]  = sp;
    frame.x[10] = argc; // a0
    frame.x[11] = argv; // a1

    #[cfg(feature = "debug")]
    {
        kprintln!("[STACK] argc: {}, argv: {:#x}", argc, argv);
        for i in 0..argc {
            let arg_ptr: usize = unsafe { *(argv as *const usize).add(i) };
            let arg_str = unsafe {
                let mut len = 0;
                while *((arg_ptr + len) as *const u8) != 0 {
                    len += 1;
                }
                let slice = slice::from_raw_parts(arg_ptr as *const u8, len);
                core::str::from_utf8_unchecked(slice)
            };
            kprintln!("[STACK]   arg[{}]: {:#x} -> {}", i, arg_ptr, arg_str);
        }
    }

    thread::Builder::new(move || start(frame))
        .pagetable(pt)
        .userproc(userproc)
        .parent_tid(thread::current().id())
        .spawn()
        .id()
}

/// Exits a process.
///
/// Panic if the current thread doesn't own a user process.
pub fn exit(_value: isize) -> ! {
    // TODO: Lab2.
    let current = thread::current();
    if current.userproc().is_none() {
        panic!("exit() called by a non-user thread");
    }
    thread::exit();
}

/// Waits for a child thread, which must own a user process.
///
/// ## Return
/// - `Some(exit_value)`
/// - `None`: if tid was not created by the current thread.
pub fn wait(tid: isize) -> Option<isize> {
    // TODO: Lab2.
    Some(0)
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
