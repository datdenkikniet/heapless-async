use core::{
    cell::UnsafeCell,
    ops::{Deref, DerefMut},
    sync::atomic::{AtomicBool, Ordering},
};

pub struct Mutex<T> {
    locked: AtomicBool,
    value: UnsafeCell<T>,
}

unsafe impl<T> Send for Mutex<T> {}
unsafe impl<T> Sync for Mutex<T> {}

impl<T> Mutex<T> {
    pub const fn new(value: T) -> Self {
        Self {
            locked: AtomicBool::new(false),
            value: UnsafeCell::new(value),
        }
    }

    pub fn try_lock(&self) -> Option<MutexGuard<T>> {
        if let Ok(_) =
            self.locked
                .compare_exchange(false, true, Ordering::SeqCst, Ordering::Relaxed)
        {
            Some(MutexGuard { lock: self })
        } else {
            None
        }
    }
}

pub struct MutexGuard<'lock, T> {
    lock: &'lock Mutex<T>,
}

impl<T> Drop for MutexGuard<'_, T> {
    fn drop(&mut self) {
        self.lock.locked.store(false, Ordering::SeqCst);
    }
}

impl<T> Deref for MutexGuard<'_, T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        // SAFETY: So long as a `LockGuard` exists, it has exclusive
        // access to the value guarded by the lock
        unsafe { &*self.lock.value.get() }
    }
}

impl<T> DerefMut for MutexGuard<'_, T> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        // SAFETY: So long as a `LockGuard` exists, it has exclusive
        // access to the value guarded by the lock
        unsafe { &mut *self.lock.value.get() }
    }
}
