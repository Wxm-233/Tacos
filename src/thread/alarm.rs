use alloc::vec::Vec;
use alloc::sync::Arc;

use crate::sync::{Mutex, Lazy};
use crate::thread::Thread;
use crate::sbi::timer::timer_ticks;
use crate::sbi::interrupt;

struct Entry {
    wake: i64,
    thread: Arc<Thread>,
}

static ALARM: Lazy<Mutex<Vec<Entry>>> = Lazy::new(|| Mutex::new(Vec::new()));

pub fn add(wake: i64, thread: Arc<Thread>) {
    let mut q = ALARM.lock();
    q.push(Entry { wake, thread });
    q.sort_by_key(|e| e.wake);
}

pub fn tick() {
    let old = interrupt::set(false);
    let now = timer_ticks();

    let mut q = ALARM.lock();
    let mut i = 0usize;

    while i < q.len() {
        if q[i].wake <= now {
            let entry = q.remove(i);
            crate::thread::wake_up(entry.thread);
        } else {
            break;
        }
    }

    interrupt::set(old);
}