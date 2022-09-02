use core::{
    cell::UnsafeCell,
    sync::atomic::{AtomicUsize, Ordering},
};

const UNLOCKED: usize = 0;
const LOCKED: usize = 1;

pub struct Lock<T> {
    lock: AtomicUsize,
    value: UnsafeCell<T>,
}

unsafe impl<T> Send for Lock<T> {}
unsafe impl<T> Sync for Lock<T> {}

impl<T> Lock<T> {
    pub fn new(value: T) -> Self {
        Self {
            lock: AtomicUsize::new(UNLOCKED),
            value: UnsafeCell::new(value),
        }
    }

    pub fn try_lock<F, R>(&self, op: F) -> Option<R>
    where
        F: FnOnce(&mut T) -> R,
    {
        if let Ok(_) =
            self.lock
                .compare_exchange(UNLOCKED, LOCKED, Ordering::SeqCst, Ordering::Relaxed)
        {
            // SAFETY: we are guaranteed to have locked `self.lock` here,
            // so we have exclusive access to &self
            let value_mut = unsafe { &mut *self.value.get() };
            let res = op(value_mut);

            self.lock.store(UNLOCKED, Ordering::SeqCst);

            Some(res)
        } else {
            None
        }
    }
}
