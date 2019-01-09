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

pub type GuardCell = RwCell<u8>;

impl<T> RwCell<T> {
    #[inline]
    pub fn new(item: T, guard: Arc<AtomicI64>) -> RwCell<T> {
        RwCell {
            item: UnsafeCell::new(item),
            guard,
        }
    }

    #[inline]
    pub fn single(item: T) -> RwCell<T> {
        RwCell {
            item: UnsafeCell::new(item),
            guard: Arc::new(AtomicI64::new(0)),
        }
    }

    #[inline]
    pub fn guard() -> GuardCell {
        RwCell {
            item: UnsafeCell::new(0u8),
            guard: Arc::new(AtomicI64::new(0)),
        }
    }

    /// Apply a function to an immutable reference of the guarded value. Slightly faster than acquiring
    /// a guard.
    #[inline]
    pub fn apply<F, R>(&self, f: F) -> R
    where
        F: FnOnce(&T) -> R,
    {
        self.acquire_read();
        let result = f(unsafe { &*self.item.get() });
        self.guard.fetch_sub(1, Ordering::Relaxed);
        result
    }

    /// Apply a function to a mutable reference of the guarded value. Slightly faster than acquiring
    /// a guard.
    #[inline]
    pub fn apply_mut<F, R>(&self, f: F) -> R
    where
        F: FnOnce(&mut T) -> R,
    {
        self.acquire_write();
        let result = f(unsafe { &mut *self.item.get() });
        self.guard.store(0, Ordering::SeqCst);
        result
    }

    /// Get read-only access to the cell. There can be multiple readers, but no concurrent writer.
    pub fn read(&self) -> ReadGuard<T> {
        self.acquire_read();
        ReadGuard {
            ptr: self.item.get(),
            guard: self.guard.clone(),
        }
    }

    /// Get read-write access to the cell. Note that there can only be one writer and no readers
    /// at a time.
    pub fn write(&self) -> RwGuard<T> {
        self.acquire_write();

        RwGuard {
            ptr: self.item.get(),
            guard: self.guard.clone(),
        }
    }

    #[inline]
    fn acquire_read(&self) {
        loop {
            let value = self.guard.load(Ordering::Acquire);
            let new = self.guard.compare_and_swap(value, value + 1, Ordering::Release);

            if new == -1 {
                panic!("Attempted to acquire read lock when a write lock is already in effect");
            } else if new == value {
                break;
            }
        }
    }

    #[inline]
    fn acquire_write(&self) {
        if self.guard.compare_and_swap(0, -1, Ordering::AcqRel) != 0 {
            panic!("Attempted to acquire a write lock while another lock is already in effect")
        }
    }

    #[inline]
    pub(crate) unsafe fn get_ptr_raw(&self) -> *mut T {
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

pub enum Access {
    Read(Arc<GuardCell>),
    Write(Arc<GuardCell>),
}

impl Access {
    fn acquire(&self) -> Box<Drop> {
        match self {
            Access::Read(access) => Box::new(access.read()),
            Access::Write(access) => Box::new(access.write()),
        }
    }
}

pub struct MultiLock {
    locks: Vec<Access>,
}

impl MultiLock {
    pub fn new(locks: Vec<Access>) -> MultiLock {
        MultiLock { locks }
    }

    pub fn acquire(&self) -> MultiBorrow {
        MultiBorrow {
            _borrows: self.locks.iter().map(|lock| lock.acquire()).collect(),
        }
    }
}

pub struct MultiBorrow {
    _borrows: Vec<Box<Drop>>,
}

unsafe impl Sync for MultiBorrow {}
unsafe impl Send for MultiBorrow {}

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

        lock.apply_mut(|e| *e = 15);
        lock.apply(|e| assert_eq!(*e, 15));

        let e = lock.read();
        assert_eq!(*e, 15);
    }

    #[test]
    fn test_multilock() {
        let cell1 = Arc::new(GuardCell::guard());
        let cell2 = Arc::new(GuardCell::guard());
        let cell3 = Arc::new(GuardCell::guard());

        let lock1 = MultiLock::new(vec![
            Access::Read(cell1.clone()),
            Access::Read(cell2.clone()),
            Access::Read(cell3.clone()),
        ]);

        let lock2 = MultiLock::new(vec![Access::Read(cell1), Access::Read(cell2), Access::Read(cell3)]);

        // All locks are read-only and can therefore be acquired
        let _guard1 = lock1.acquire();
        let _guard2 = lock2.acquire();
    }

    #[test]
    #[should_panic(expected = "Attempted to acquire read lock when a write lock is already in effect")]
    fn test_multilock_fail() {
        let cell1 = Arc::new(GuardCell::guard());
        let cell2 = Arc::new(GuardCell::guard());

        let lock1 = MultiLock::new(vec![Access::Read(cell1.clone()), Access::Write(cell2.clone())]);
        let lock2 = MultiLock::new(vec![Access::Read(cell2)]);

        // Fails while trying to acquire cell2 for both read and write.
        let _guard1 = lock1.acquire();
        let _guard2 = lock2.acquire();
    }
}
