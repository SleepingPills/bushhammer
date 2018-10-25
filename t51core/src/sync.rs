use std::cell::UnsafeCell;
use std::ops::Deref;
use std::ops::DerefMut;
use std::sync::atomic::{AtomicI64, Ordering};
use std::sync::Arc;

/// A fail-fast threadsafe read-write cell with similar semantics to a RefCell. There can be any
/// number of readers, or a single writer. Any combination of readers and writers will cause
/// a panic.
pub struct RwCell<T> {
    item: UnsafeCell<T>,
    guard: Arc<AtomicI64>,
}

impl<T> RwCell<T> {
    pub fn new(item: T, guard: Arc<AtomicI64>) -> RwCell<T> {
        RwCell {
            item: UnsafeCell::new(item),
            guard,
        }
    }

    /// Get read-only access to the cell. There can be multiple readers, but no concurrent writer.
    pub fn read(&self) -> ReadGuard<T> {
        loop {
            let value = self.guard.load(Ordering::Acquire);

            let new = self.guard.compare_and_swap(value, value + 1, Ordering::Release);

            if new == -1 {
                panic!("Attempted to acquire read lock when a write lock is already in effect")
            } else if new == value {
                break ReadGuard {
                    ptr: self.item.get(),
                    guard: self.guard.clone(),
                };
            }
        }
    }

    /// Get read-write access to the cell. Note that there can only be one writer and no readers
    /// at a time.
    pub fn write(&self) -> RwGuard<T> {
        let value = self.guard.load(Ordering::Acquire);

        let new = self.guard.compare_and_swap(value, -1, Ordering::Release);

        if new == 0 {
            return RwGuard {
                ptr: self.item.get(),
                guard: self.guard.clone(),
            };
        } else {
            panic!("Attempted to acquire a write lock while another lock is already in effect")
        }
    }

    #[inline]
    pub(crate) unsafe fn get_ptr(&self) -> *mut T {
        self.item.get()
    }
}

unsafe impl<T> Send for RwCell<T> {}

unsafe impl<T> Sync for RwCell<T> {}

pub struct ReadGuard<T> {
    ptr: *const T,
    guard: Arc<AtomicI64>,
}

unsafe impl<T> Send for ReadGuard<T> {}

unsafe impl<T> Sync for ReadGuard<T> {}

impl<T> Drop for ReadGuard<T> {
    #[inline]
    fn drop(&mut self) {
        self.guard.fetch_sub(1, Ordering::Relaxed);
    }
}

impl<T> Deref for ReadGuard<T> {
    type Target = T;

    #[inline]
    fn deref(&self) -> &T {
        unsafe { &*self.ptr }
    }
}

pub struct RwGuard<T> {
    ptr: *mut T,
    guard: Arc<AtomicI64>,
}

unsafe impl<T> Send for RwGuard<T> {}

unsafe impl<T> Sync for RwGuard<T> {}

impl<T> Drop for RwGuard<T> {
    #[inline]
    fn drop(&mut self) {
        self.guard.store(0, Ordering::SeqCst);
    }
}

impl<T> Deref for RwGuard<T> {
    type Target = T;

    #[inline]
    fn deref(&self) -> &T {
        unsafe { &*self.ptr }
    }
}

impl<T> DerefMut for RwGuard<T> {
    #[inline]
    fn deref_mut(&mut self) -> &mut T {
        unsafe { &mut *self.ptr }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_rwcell() {
        let lock = RwCell {
            item: UnsafeCell::new(5),
            guard: Arc::new(AtomicI64::new(0)),
        };

        {
            let a = lock.read();
            {
                let b = lock.read();
                {
                    let c = lock.read();

                    assert_eq!(*a, 5);
                    assert_eq!(*b, 5);
                    assert_eq!(*c, 5);
                }
            }
        }

        {
            let mut d = lock.write();

            assert_eq!(*d, 5);
            *d = 10;
            assert_eq!(*d, 10);
        }

        let e = lock.read();

        assert_eq!(*e, 10);
    }
}
