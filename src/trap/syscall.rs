//! Syscall handlers
//!

#![allow(dead_code)]

/* -------------------------------------------------------------------------- */
/*                               SYSCALL NUMBER                               */
/* -------------------------------------------------------------------------- */

use alloc::string::String;
use alloc::vec::Vec;

use crate::alloc::vec;
use crate::fs::File;
use crate::mem::palloc::UserPool;
use crate::mem::userbuf;
use crate::mem::{PTEFlags, PageTable, PG_SHIFT, PG_SIZE};
use crate::thread::current;

use crate::{
    OsError,
    fs::{FileSys, disk::DISKFS},
    io::{Seek, SeekFrom, Write, Read},
    mem::{PG_MASK, in_kernel_space, pagetable},
    sbi::{console_getchar, shutdown},
    thread,
    userproc::{self, execute}
};

const SYS_HALT:     usize = 1;
const SYS_EXIT:     usize = 2;
const SYS_EXEC:     usize = 3;
const SYS_WAIT:     usize = 4;
const SYS_REMOVE:   usize = 5;
const SYS_OPEN:     usize = 6;
const SYS_READ:     usize = 7;
const SYS_WRITE:    usize = 8;
const SYS_SEEK:     usize = 9;
const SYS_TELL:     usize = 10;
const SYS_CLOSE:    usize = 11;
const SYS_FSTAT:    usize = 12;

const SYS_MMAP:     usize = 13;
const SYS_MUNMAP:   usize = 14;

const O_RDONLY:     usize = 0;
const O_WRONLY:     usize = 0x001;
const O_RDWR:       usize = 0x002;
const O_CREATE:     usize = 0x200;
const O_TRUNC:      usize = 0x400;

fn valid_ptr(ptr: usize) -> bool {
    !in_kernel_space(ptr)
        && match &thread::current().pagetable {
            Some(pagetable) => 
                match pagetable.lock().get_pte(ptr) {
                    Some(entry) => entry.is_valid(),
                    _ => false,
                },
                _ => false,
        }
}

fn ptr2string(mut ptr: usize) -> Option<String> {
    let mut str = String::new();

    // no! why can't I break a value from a while-loop.
    loop {
        if !valid_ptr(ptr) {
            break None;
        }
        let c = unsafe { (ptr as *mut u8).read() };
        if c == b'\0' {
            break Some(str);
        }
        str.push(c as char);
        ptr += 1;
    }
}

pub fn syscall_handler(id: usize, args: [usize; 3]) -> isize {
    // TODO: LAB2 impl
    match id {
        SYS_HALT => {
            shutdown();
        }
        SYS_EXIT => {
            userproc::exit(args[0] as isize);
        }
        SYS_EXEC => {
            let name = match ptr2string(args[0]) {
                Some(str) => str,
                _ => return -1,
            };

            let file = match DISKFS.open(name.as_str().into()) {
                Ok(f) => f,
                _ => return -1,
            };

            let mut argv = Vec::new();
            let mut ptr = args[1];
            loop {
                if !valid_ptr(ptr) {
                    break -1;
                }
                let val = unsafe {
                    (ptr as *const usize).read()
                };
                if val == 0 {
                    break execute(file, argv);
                }
                let str = match ptr2string(val) {
                    Some(str) => str,
                    _ => break -1,
                };
                argv.push(str);

                ptr += core::mem::size_of::<usize>();
            }
        }
        SYS_WAIT => {
            match userproc::wait(args[0] as isize) {
                Some(x) => x,
                _ => -1,
            }
        }
        SYS_REMOVE => {
            let name = match ptr2string(args[0]) {
                Some(name) => name,
                _ => return -1,
            };
            if name.len() == 0 {
                return -1;
            }
            match DISKFS.remove(name.as_str().into()) {
                Ok(_) => 0,
                _ => -1,
            }
        }
        SYS_OPEN => {
            let name = match ptr2string(args[0]) {
                Some(name) => name,
                _ => return -1,
            };
            if name.len() == 0 {
                return -1;
            }
            let flag = args[1];
            let file = match if (flag & O_TRUNC) != 0 {
                DISKFS.create(name.as_str().into())
            } else {
                match DISKFS.open(name.as_str().into()) {
                    Err(OsError::NoSuchFile) => {
                        if (flag & O_CREATE) != 0 {
                            DISKFS.create(name.as_str().into())
                        } else {
                            return -1;
                        }
                    }
                    other => other,
                }
            } {
                Ok(file) => file,
                _ => {
                    return -1;
                }
            };
            thread::current().fdlist.lock().open(file, flag)
        }
        SYS_READ => {
            let fd = args[0] as isize;
            if fd == 1 || fd == 2 {
                return -1;
            }
            let ptr = args[1] as usize;
            let raw_ptr = ptr as *mut u8;
            let size = args[2] as usize;

            for i in 0..size {
                if !valid_ptr(ptr + i) {
                    return -1;
                }
            }
            let buf: &mut [u8] = unsafe { core::slice::from_raw_parts_mut(raw_ptr, size) };

            if fd == 0 {
                for i in 0..size {
                    buf[i] = console_getchar() as u8;
                }
                return size as isize;
            }
            // why will a borrow problem happen here when I write a long statement??
            let current = thread::current();
            let mut fdlist = current.fdlist.lock();
            let file = match fdlist.get_by_fd(fd) {
                Some(fdinfo) if (fdinfo.flag & O_WRONLY) == 0 => &mut fdinfo.file,
                _ => return -1,
            };

            match file.read(buf) {
                Ok(ret) => ret as isize,
                _ => -1,
            }
        }
        SYS_WRITE => {
            let fd = args[0] as isize;
            let ptr = args[1];
            let raw_ptr = ptr as *mut u8;
            let size = args[2];
            if fd == 0 {
                return -1;
            }
            for i in 0..size {
                if !valid_ptr(ptr + i) {
                    return -1;
                }
            }
            let buf: &[u8] = unsafe { core::slice::from_raw_parts(raw_ptr, size) };
            if fd == 1 || fd == 2 {
                for i in 0..size {
                    kprint!("{}", buf[i] as char);
                }
                size as isize
            } else {
                let current = thread::current();
                let mut fdlist = current.fdlist.lock();
                let file = match fdlist.get_by_fd(fd) {
                    Some(fd_info) if (fd_info.flag & (O_WRONLY | O_RDWR)) != 0 => &mut fd_info.file,
                    _ => return -1,
                };
                match file.write(buf) {
                    Ok(x) => x as isize,
                    _ => -1,
                }
            }
        }
        SYS_SEEK => {
            let fd = args[0] as isize;
            let pos = args[1];
            if fd >= 0 && fd <= 2 {
                return -1;
            }

            let current = thread::current();
            let mut fdlist = current.fdlist.lock();
            let file = match fdlist.get_by_fd(fd) {
                Some(x) => &mut x.file,
                _ => return -1,
            };

            let _ = file.pos().map(|x| *x = pos);
            0
        }
        SYS_TELL => {
            let fd = args[0] as isize;
            if fd >= 0 && fd <= 2 {
                return -1;
            }

            let current = thread::current();
            let mut fdlist = current.fdlist.lock();
            let file = match fdlist.get_by_fd(fd) {
                Some(x) => &mut x.file,
                _ => return -1,
            };

            *file.pos().unwrap() as isize
        }
        SYS_FSTAT => {
            let fd = args[0] as isize;
            if fd >= 0 && fd <= 2 {
                return -1;
            }

            let current = thread::current();
            let mut fdlist = current.fdlist.lock();
            let file = match fdlist.get_by_fd(fd) {
                Some(x) if (x.flag & O_WRONLY) == 0 => &mut x.file,
                _ => return -1,
            };

            let size = file.len().unwrap();
            let ptr = args[1];
            if valid_ptr(ptr) {
                unsafe {
                    (ptr as *mut usize).write(file.inum());
                    ((ptr + core::mem::size_of::<usize>()) as *mut usize).write(size); 
                }
            }
            0
        }
        SYS_CLOSE => {
            let fd = args[0] as isize;
            if fd >= 0 && fd <= 2 {
                return 0;
            }

            let current = thread::current();
            if !current
                .fdlist
                .lock()
                .list
                .iter()
                .fold(false, |x, y| x || y.fd == fd) 
            {
                return -1;
            }
            current.fdlist.lock().list.retain(|x| x.fd != fd);
            0
        }
        SYS_MMAP => {
            let fd = args[0] as isize;
            let va = args[1];
            if fd >= 0 && fd <= 2 {
                return -1; // not mappable
            }
            let mut flags = PTEFlags::R | PTEFlags::U;
            let current = thread::current();
            let mut fdlist = current.fdlist.lock();
            let file = match fdlist.get_by_fd(fd) {
                Some(x) => {
                    if x.flag & (O_WRONLY | O_RDWR) != 0 {
                        flags |= PTEFlags::W;
                    }
                    &mut x.file
                }
                _ => return -1, // file descriptor not found
            };
            if file.len().unwrap() == 0 {
                return -1; // empty file not mappable
            }
            if (va & 0xFFF) != 0 {
                return -1; // page not aligned
            }
            if va == 0 {
                return -1; // null pointer not mappable
            }

            let pt = unsafe { PageTable::effective_pagetable() };
            for i in (0..file.len().unwrap()).step_by(PG_SIZE) {
                if let Some(entry) = pt.get_pte(va + i) {
                    if entry.is_valid() {
                        return -1; // page already mapped
                    }
                }
            }
            let size = file.len().unwrap();
            let page_count = (size + PG_SIZE - 1) >> PG_SHIFT;

            if !current
                .supplementary_pagetable
                .lock()
                .va_range_check(va, va + (page_count << PG_SHIFT)) {
                return -1; // invalid virtual address range
            }

            if !current
                .mapping_table
                .lock()
                .va_range_check(va, va + (page_count << PG_SHIFT)) {
                return -1; // already mapped
            }

            thread::current()
                .mapping_table
                .lock()
                .map(file.clone(), 0, va, size, size, flags)
        }
        SYS_MUNMAP => {
            let mapid = args[0] as isize;
            if mapid < 0 {
                return -1; // invalid mapping id
            }
            let pt = unsafe { PageTable::effective_pagetable() };
            let current = thread::current();
            let mut mapping_table = current
                .mapping_table
                .lock();
            let mapinfo = match mapping_table
                .list
                .iter_mut()
                .find(|mapinfo| mapinfo.mapid == mapid) {
                    Some(mapinfo) => mapinfo,
                    _ => return -1,
                };
            for i in (0..mapinfo.memsize).step_by(PG_SIZE) {
                if let Some(entry) = pt.get_pte_mut(mapinfo.va + i) {
                    if entry.is_valid() && entry.is_dirty() {
                        mapinfo
                            .file
                            .as_mut()
                            .unwrap()
                            .seek(SeekFrom::Start((mapinfo.offset + i) as usize))
                            .unwrap();
                        let size = (mapinfo.memsize - i).min(PG_SIZE);

                        let buf = unsafe {
                            ((mapinfo.va + i) as *const [u8; PG_SIZE]).as_ref().unwrap()
                        };
                        mapinfo.file.as_mut().unwrap().write(&buf[..size]).unwrap();
                    }
                    unsafe {
                        UserPool::dealloc_pages((entry.pa().into_va()) as *mut u8, 1);
                    }
                    entry.set_invalid();
                }
            }
            pt.activate();
            thread::current()
                .mapping_table
                .lock()
                .list
                .retain(|mapinfo| mapinfo.mapid != mapid);
            0
        }
        _ => {
            panic!("unknown syscall");
        }
    }
}
