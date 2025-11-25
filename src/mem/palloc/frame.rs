use alloc::collections::VecDeque;

use crate::{mem::{PTEFlags, USER_POOL_LIMIT, VM_OFFSET}, trap::Frame};

use core::array;
use core::cmp::min;
use alloc::sync::{Arc, Weak};
use crate::sync::{Intr, Lazy, Mutex, Primitive};
use crate::fs::disk::Swap;
use crate::io::{Seek, SeekFrom, Write, Read};
use crate::mem::utils::*;
use crate::thread::{current, Thread};

pub struct FrameInfo {
    pub thread: Weak<Thread>,
    pub va: usize,
    pub flags: PTEFlags,
}

impl FrameInfo {
    pub fn new(thread: Weak<Thread>, va: usize, flags: PTEFlags) -> Self {
        Self { thread, va, flags }
    }
}

pub struct FrameTable {
    pub start: usize,
    pub end: usize,
    pub entries: [Option<FrameInfo>; USER_POOL_LIMIT],
    pub used_pages: VecDeque<usize>,
}

impl FrameTable {
    pub fn new() -> Self {
        Self {
            start: 0,
            end: 0,
            entries: array::from_fn(|_| None),
            used_pages: VecDeque::new(),
        }
    }

    pub fn set_range(&mut self, start: usize, end: usize) {
        self.start = start - VM_OFFSET;
        self.end = end - VM_OFFSET;
    }
}

pub struct GlobalFrameTable(Lazy<Mutex<FrameTable, Primitive>>);

impl GlobalFrameTable {
    pub fn init(start: usize, end: usize) {
        Self::instance().lock().set_range(start, end);
    }

    pub fn map(pa: usize, va: usize, flag: PTEFlags) {
        let mut frame_table = Self::instance().lock();

        let index = (pa - frame_table.start) >> PG_SHIFT;
        match frame_table.entries.get(index).unwrap() {
            Some(x) => {
                x.thread.upgrade();
            }
            _ => (),
        };
        frame_table.used_pages.push_back(index);
        *frame_table.entries.get_mut(index).unwrap() = 
            Some(FrameInfo::new(Arc::downgrade(&current()), va, flag));
    }

    pub fn destroy(pa: usize) {
        let mut frame_table = Self::instance().lock();

        let index = (pa - frame_table.start) >> PG_SHIFT;
        *frame_table.entries.get_mut(index).unwrap() = None;
    }

    pub fn start() -> usize {
        Self::instance().lock().start
    }

    pub fn end() -> usize {
        Self::instance().lock().end
    }

    pub fn instance() -> &'static Mutex<FrameTable, Primitive> {
        static FRAMETABLE: GlobalFrameTable = 
            GlobalFrameTable(Lazy::new(|| Mutex::new(FrameTable::new())));

        &FRAMETABLE.0
    }
}