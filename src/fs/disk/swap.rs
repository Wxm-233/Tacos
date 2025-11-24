//! Swap file.
//!
// Swap may not be used.
#![allow(dead_code)]
use super::DISKFS;
use crate::fs::{File, FileSys};
use crate::io::Seek;
use crate::mem::PG_SIZE;
use crate::sync::{Lazy, Mutex, MutexGuard, Primitive};

use crate::io::{Read, SeekFrom, Write};
use alloc::vec::Vec;

pub struct Swap;

static SWAPFILE: Lazy<Mutex<File, Primitive>> = Lazy::new(|| {
    Mutex::new(
        DISKFS
            .open(".glbswap".into())
            .expect("swap file \".glbswap\" should exist"),
    )
});

static SWAPFILE_VEC: Lazy<Mutex<Vec<usize>, Primitive>> = Lazy::new(|| {
    let mut v = Vec::new();
    for i in (0..Swap::len()).step_by(PG_SIZE) {
        v.push(i);
    }
    Mutex::new(v)
});

impl Swap {
    pub fn len() -> usize {
        SWAPFILE.lock().len().unwrap()
    }

    pub fn page_num() -> usize {
        // Round down.
        Self::len() / PG_SIZE
    }

    /// TODO: Design high-level interfaces, or do in lab3?
    pub fn lock() -> MutexGuard<'static, File, Primitive> {
        SWAPFILE.lock()
    }

    pub fn new_page() -> usize {
        let mut vec = SWAPFILE_VEC.lock();
        vec.pop().unwrap()
    }

    pub fn push_page(page: usize) {
        let mut vec = SWAPFILE_VEC.lock();
        vec.push(page);
    }

    pub fn read_page(pos: usize, buf: &mut [u8]) -> usize {
        let mut file = SWAPFILE.lock();
        if (pos >= file.len().unwrap()) {
            panic!("read invalid swap page");
        }
        file.seek(SeekFrom::Start(pos as usize)).unwrap();
        file.read(buf).unwrap()
    }

    pub fn write_page(pos: usize, buf: &[u8]) {
        let mut file = SWAPFILE.lock();
        if (pos >= file.len().unwrap()) {
            panic!("write invalid swap page");
        }
        file.seek(SeekFrom::Start(pos as usize)).unwrap();
        file.write(buf).unwrap();
    }
}
