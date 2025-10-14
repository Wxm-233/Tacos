use alloc::sync::Arc;
use core::cell::RefCell;

use core::sync::atomic::AtomicIsize;
use core::fmt::{self, Debug};

use crate::sbi::interrupt;

use crate::sync::{Lock, Primitive, Semaphore};
use crate::thread::{self, current, schedule, Thread};

use alloc::vec::Vec;
use core::borrow::{BorrowMut, Borrow};

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

        # [cfg(feature = "debug")]
        {
            kprintln!("Before acquired, it's a {:?}", self);
            kprintln!("Before acquiring, current thread {:?} with priority {:?}",
                thread::current(),
                thread::current().effective_priority());
        }

        if let Some(donating) = self.holder.borrow().clone() {
            *thread::current().donating.lock() = Some(donating.clone());
            donating.donators.lock().push(thread::current());
        }
        thread::current().donate();
        self.waiting_list.borrow_mut().push(thread::current());

        self.inner.down();

        self.holder.borrow_mut().replace(thread::current());
        *thread::current().donating.lock() = None;
        self.waiting_list.borrow_mut().retain(|t| t.id() != thread::current().id());

        # [cfg(feature = "debug")]
        {
            kprintln!("After acquiring, it's a {:?}", self);
            kprintln!("After acquiring, current thread {:?} with priority {:?}", 
                thread::current(), 
                thread::current().effective_priority());
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
        
        # [cfg(feature = "debug")]
        {
            kprintln!("Before releasing, it's a {:?}", self);
            kprintln!("Before releasing, current thread {:?} with priority {:?}",
                thread::current(), 
                thread::current().effective_priority());
        }
        
        self.holder.borrow_mut().take().unwrap();

        current().donators.lock().retain(|t| {
            for waiting in self.waiting_list.borrow().iter() {
                if Arc::ptr_eq(t, waiting) {
                    return false;
                }
            }
            true
        });

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
