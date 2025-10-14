use alloc::sync::Arc;
use core::cell::RefCell;

use core::sync::atomic::AtomicIsize;
use core::fmt::{self, Debug};

use crate::sbi::interrupt;

use crate::sync::{Lock, Semaphore};
use crate::thread::{self, schedule, Thread};

use alloc::vec::Vec;

/// Sleep lock. Uses [`Semaphore`] under the hood.
#[derive(Clone)]
pub struct Sleep {
    inner: Semaphore,
    holder: RefCell<Option<Arc<Thread>>>,
    waiting_list: RefCell<Vec<Arc<Thread>>>,
    lid: isize,
}

impl Sleep {
    pub fn get_holder(&self) -> RefCell<Option<Arc<Thread>>> {
        self.holder.clone()
    }
}

impl Debug for Sleep {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_fmt(format_args!(
            "lock{:?}(holder={:?})[waiting_list={:?}]",
            self.lid,
            self.holder.borrow(),
            self.waiting_list.borrow()
        ))
    }
}

impl Default for Sleep {
    fn default() -> Self {

        static LID: AtomicIsize = AtomicIsize::new(0);

        Self {
            inner: Semaphore::new(1),
            holder: Default::default(),
            waiting_list: Default::default(),
            lid: LID.fetch_add(1, core::sync::atomic::Ordering::SeqCst),
        }
    }
}

impl Lock for Sleep {
    fn acquire(&self) {
        let old = interrupt::set(false);

        *thread::current().required_lock.lock() = Some(self.clone());
        thread::current().donate();
        self.waiting_list.borrow_mut().push(thread::current());

        # [cfg(feature = "debug")]
        {
            kprintln!("Before acquired, {:?}'s waiting list is {:?}", self, self.waiting_list.borrow());
        }

        self.inner.down();

        self.holder.borrow_mut().replace(thread::current());
        *thread::current().required_lock.lock() = None;
        thread::current().holding_locks.lock().push(self.clone().into());
        self.waiting_list.borrow_mut().retain(|t| !Arc::ptr_eq(t, &thread::current()));

        # [cfg(feature = "debug")]
        {
            kprintln!("After acquired, {:?}'s waiting list is {:?}", self, self.waiting_list.borrow());
            kprintln!("Current thread {:?} with priority {:?} has acquired {:?}", thread::current(), thread::current().effective_priority(), self);
        }
        
        interrupt::set(old);

        schedule();
    }

    fn release(&self) {
        assert!(Arc::ptr_eq(
            self.holder.borrow().as_ref().unwrap(),
            &thread::current()
        ));
        let old = interrupt::set(false);
        self.holder.borrow_mut().take().unwrap();

        # [cfg(feature = "debug")]
        {
            kprintln!("Before releasing, current thread {:?} with priority {:?} is releasing {:?}", thread::current(), thread::current().effective_priority(), self);
            kprint!("Current thread is holding locks:");
            for lock in thread::current().holding_locks.lock().iter() {
                kprint!("{:?}", lock);
            }
            kprintln!();
            kprintln!("This lock is {:?}", self);
        }
        
        thread::current().holding_locks.lock().retain(|x| x.lid != self.lid);

        thread::current().recompute_priority();
        interrupt::set(old);

        self.inner.up();
    }
}

impl Sleep {
    pub fn get_priority(&self) -> u32 {

        #[cfg(feature = "debug")]
        {
            kprintln!("[SLEEP] waiting_list of {:?} is {:?}", self, self.waiting_list.borrow());
        }

        self.waiting_list
            .borrow()
            .iter()
            .map(|t| t.effective_priority())
            .max()
            .unwrap_or(0)

        // self.inner.get_priority()
    }
}

unsafe impl Sync for Sleep {}
