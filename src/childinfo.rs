pub use crate::sync::sema::Semaphore;
use alloc::sync::Arc;
use thread::Thread;

#[derive(Clone)]
pub struct ChildInfo {
    pub tid: isize,
    pub name: &'static str,
    pub exit_code: Option<isize>,
    pub is_waiting: bool,
    pub wait_sema: Arc<Semaphore>,
    pub ptr: Option<Arc<Thread>>,
}

impl ChildInfo {
    pub fn new(
        tid: isize,
        name: &'static str,
        exit_code: Option<isize>,
        is_waiting: bool,
        ptr: Arc<Thread>,
    ) -> Self {
        ChildInfo {
            tid,
            name,
            exit_code,
            is_waiting,
            wait_sema: Arc::new(Semaphore::new(0)),
            ptr: Some(ptr),
        }
    }
}