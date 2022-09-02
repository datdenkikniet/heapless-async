use core::{
    cell::UnsafeCell,
    sync::atomic::{AtomicBool, Ordering},
};

pub struct Lock<T> {
    locked: AtomicBool,
    value: UnsafeCell<T>,
}

unsafe impl<T> Send for Lock<T> {}
unsafe impl<T> Sync for Lock<T> {}

impl<T> Lock<T> {
    pub const fn new(value: T) -> Self {
        Self {
            locked: AtomicBool::new(false),
            value: UnsafeCell::new(value),
        }
    }

    pub fn try_lock<F, R>(&self, op: F) -> Option<R>
    where
        F: FnOnce(&mut T) -> R,
    {
        if let Ok(_) =
            self.locked
                .compare_exchange(false, true, Ordering::SeqCst, Ordering::Relaxed)
        {
            // SAFETY: we are guaranteed to have locked `self.lock` here,
            // so we have exclusive access to &self
            let value_mut = unsafe { &mut *self.value.get() };
            let res = op(value_mut);

            self.locked.store(false, Ordering::SeqCst);

            Some(res)
        } else {
            None
        }
    }
}
