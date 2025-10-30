//! User process.
//!

mod load;

use alloc::string::String;
use alloc::vec::Vec;
use core::arch::asm;
use core::convert::TryInto;
use core::mem::MaybeUninit;
use core::ptr::write_bytes;
use riscv::register::sstatus;

use ptr::copy_nonoverlapping;

use crate::fs::File;
use crate::mem::pagetable::KernelPgTable;
use crate::thread;
use crate::trap::{trap_exit_u, Frame};

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
    
    let mut sp = exec_info.init_sp;
    let mut arg_ptrs = Vec::new();
    let mut total_size = 0;

    for arg in argv.iter().rev() {
        let arg_bytes = arg.as_bytes();
        let arg_len = arg_bytes.len();
        total_size += arg_len + 1; // +1 for NULL terminator

        if total_size > 4096 {
            break; // Limit reached
        }

        sp -= (arg_len + 1) as usize;

        unsafe {
            copy_nonoverlapping(arg_bytes.as_ptr(), sp as *mut u8, arg_len);
            write_bytes((sp + arg_len) as *mut u8, 0, 1); // NULL terminator
        }
        arg_ptrs.push(sp);
    }

    arg_ptrs.push(0);

    for &ptr in arg_ptrs.iter().rev() {
        sp -= core::mem::size_of::<usize>();
        unsafe {
            write_bytes(sp as *mut usize, ptr.try_into().unwrap(), core::mem::size_of::<usize>());
        }
    }

    let argc = argv.len();
    let argv: usize = sp;

    frame.x[10] = argc; // a0
    frame.x[11] = argv; // a1

    #[cfg(feature = "debug")]
    kprintln!(
        "[PROCESS] The process to be executed has argc: {}, argv: {:?}",
        argc,
        arg_ptrs
    );

    thread::Builder::new(move || start(frame))
        .pagetable(pt)
        .userproc(userproc)
        .spawn()
        .id()
}

/// Exits a process.
///
/// Panic if the current thread doesn't own a user process.
pub fn exit(_value: isize) -> ! {
    // TODO: Lab2.
    thread::exit();
}

/// Waits for a child thread, which must own a user process.
///
/// ## Return
/// - `Some(exit_value)`
/// - `None`: if tid was not created by the current thread.
pub fn wait(_tid: isize) -> Option<isize> {
    // TODO: Lab2.
    Some(-1)
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
