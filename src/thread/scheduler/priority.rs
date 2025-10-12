use alloc::collections::VecDeque;
use alloc::sync::Arc;
use crate::thread::Status;

use crate::thread::{Schedule, Thread};

/// Priority scheduler.
#[derive(Default)]
pub struct PriorityScheduler(VecDeque<Arc<Thread>>);

impl Schedule for PriorityScheduler {
    fn register(&mut self, thread: Arc<Thread>) {
        self.0.push_front(thread)
    }

    fn schedule(&mut self, current: Arc<Thread>) -> Option<Arc<Thread>> {
        self.0.make_contiguous().sort_by(|a, b| a.effective_priority().cmp(&b.effective_priority()));

        let cur_prio = current.effective_priority();

        if current.status() == Status::Running && cur_prio > self.0.back().map(|t| t.effective_priority()).unwrap_or(0) {
            return None;
        }
        self.0.pop_back()
    }
}