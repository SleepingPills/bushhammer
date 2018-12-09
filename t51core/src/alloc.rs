use std::any::TypeId;
use std::marker::Unsize;
use std::ops::{Deref, DerefMut};
use std::ptr::NonNull;

/// Dynamic pointer type that encapsulates a non-null pointer and can be cast with a type check.
pub struct DynPtr(NonNull<()>, TypeId);

impl DynPtr {
    /// New dynamic non-null pointer.
    #[inline]
    pub fn new<T>(inst: *const T) -> DynPtr
    where
        T: 'static,
    {
        DynPtr(
            NonNull::new(inst as *mut T).expect("Dynamic pointer can't be null").cast::<()>(),
            TypeId::of::<T>(),
        )
    }

    /// New dynamic non-null pointer, skipping the null check.
    #[inline]
    pub unsafe fn new_unchecked<T>(inst: *const T) -> DynPtr
    where
        T: 'static,
    {
        DynPtr(NonNull::new_unchecked(inst as *mut T).cast::<()>(), TypeId::of::<T>())
    }

    /// Cast the pointer to the specified type. Will panic if the desired type does not match
    /// the original.
    #[inline]
    pub fn cast_checked<T>(&self) -> NonNull<T>
    where
        T: 'static,
    {
        if TypeId::of::<T>() != self.1 {
            panic!("Type mismatch")
        }

        self.0.cast::<T>()
    }

    /// Cast the pointer to the specified type and return a raw pointer. Will panic if the
    /// desired type does not match the original.
    #[inline]
    pub fn cast_checked_raw<T>(&self) -> *mut T
        where
            T: 'static,
    {
        self.cast_checked::<T>().as_ptr()
    }
}

impl Deref for DynPtr {
    type Target = NonNull<()>;

    #[inline]
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl DerefMut for DynPtr {
    #[inline]
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}

/// A pool allocator that keeps all items in an efficient dense vector. New elements will be
/// used to fill up holes created by previous reclamation.
#[derive(Debug)]
pub struct VecPool<T> {
    store: Vec<T>,
    queue: Vec<usize>,
}

impl<T> VecPool<T> {
    pub fn new() -> Self {
        VecPool {
            store: Vec::new(),
            queue: Vec::new(),
        }
    }

    /// Reclaim the value at supplied index.
    #[inline]
    pub fn reclaim(&mut self, index: usize) {
        self.queue.push(index);
    }

    /// Push a new value into the storage. The pool will attempt to use any reclaimed slots
    /// before appending to the end of the storage vector.
    #[inline]
    pub fn push(&mut self, value: T) -> usize {
        if let Some(index) = self.queue.pop() {
            self.store[index] = value;
            index
        } else {
            self.store.push(value);
            self.store.len() - 1
        }
    }

    #[inline]
    pub fn len(&self) -> usize {
        self.store.len()
    }

    #[inline]
    pub fn get(&self, index: usize) -> &T {
        &self.store[index]
    }

    #[inline]
    pub fn get_mut(&mut self, index: usize) -> &mut T {
        &mut self.store[index]
    }

    #[inline]
    pub unsafe fn get_unchecked(&self, index: usize) -> &T {
        self.store.get_unchecked(index)
    }

    #[inline]
    pub unsafe fn get_unchecked_mut(&mut self, index: usize) -> &mut T {
        self.store.get_unchecked_mut(index)
    }

    #[inline]
    pub unsafe fn get_buffer_ptr(&self) -> *const T {
        self.store.as_ptr()
    }
}

/// A slot based allocator that supports testing for the presence of values in the pool.
#[derive(Debug)]
pub struct SlotPool<T> {
    store: Vec<Option<T>>,
    queue: Vec<usize>,
}

impl<T> SlotPool<T> {
    pub fn new() -> Self {
        SlotPool {
            store: Vec::new(),
            queue: Vec::new(),
        }
    }

    /// Reclaim the value at supplied index.
    #[inline]
    pub fn reclaim(&mut self, index: usize) -> Option<T> {
        self.queue.push(index);
        self.store[index].take()
    }

    /// Get an immutable reference to the item at the supplied index.
    #[inline]
    pub fn get(&self, index: usize) -> Option<&T> {
        if index >= self.store.len() {
            return None;
        }

        unsafe {
            let slot = self.store.get_unchecked(index);
            match slot {
                Some(value) => Some(&value),
                _ => None,
            }
        }
    }

    /// Get a mutable reference to the item at the supplied index.
    #[inline]
    pub fn get_mut(&mut self, index: usize) -> Option<&mut T> {
        if index >= self.store.len() {
            return None;
        }

        unsafe {
            let slot = self.store.get_unchecked_mut(index);
            match slot {
                Some(ref mut value) => Some(value),
                _ => None,
            }
        }
    }

    /// Push a new value into the storage. The pool will attempt to use any reclaimed slots
    /// before appending to the end of the storage vector.
    #[inline]
    pub fn push(&mut self, value: T) -> usize {
        let wrapped = Some(value);

        if let Some(index) = self.queue.pop() {
            self.store[index] = wrapped;
            index
        } else {
            self.store.push(wrapped);
            self.store.len() - 1
        }
    }

    /// Get the location of the next insert.
    #[inline]
    pub fn peek_index(&self) -> usize {
        match self.queue.last() {
            Some(index) => *index,
            _ => self.store.len(),
        }
    }
}

/// Trait for retrieving the pointer to the actual object implementing the trait
pub trait DynVecOps {
    unsafe fn get_inner_ptr(&self) -> DynPtr;
}

impl<T> DynVecOps for Vec<T>
where
    T: 'static,
{
    unsafe fn get_inner_ptr(&self) -> DynPtr {
        DynPtr::new_unchecked(self as *const Vec<T>)
    }
}

/// Dynamically typed, heap allocated vector
pub struct DynVec<T>
where
    T: ?Sized + DynVecOps,
{
    inst: Box<T>,
    inst_ptr: DynPtr,
}

impl<T> DynVec<T>
where
    T: 'static + ?Sized + DynVecOps,
{
    pub fn new<R>() -> DynVec<T>
    where
        Vec<R>: 'static + Unsize<T>,
    {
        unsafe {
            let inst = Box::<Vec<R>>::new(Vec::new());
            let inst_ptr = inst.get_inner_ptr();

            DynVec {
                inst,
                inst_ptr,
            }
        }
    }

    /// Retrieve a reference to the inner typed vector. Panics if there is a type mismatch.
    #[inline]
    pub fn cast_vector<R>(&self) -> &Vec<R>
    where
        Vec<R>: 'static + Unsize<T>,
    {
        unsafe { &*self.inst_ptr.cast_checked::<Vec<R>>().as_ptr() }
    }

    /// Retrieve a mutable reference to the inner typed vector. Panics if there is a type mismatch.
    #[inline]
    pub fn cast_mut_vector<R>(&mut self) -> &mut Vec<R>
    where
        Vec<R>: 'static + Unsize<T>,
    {
        unsafe { &mut *self.inst_ptr.cast_checked::<Vec<R>>().as_ptr() }
    }
}

impl<T> Deref for DynVec<T>
where
    T: 'static + ?Sized + DynVecOps,
{
    type Target = Box<T>;

    fn deref(&self) -> &Self::Target {
        &self.inst
    }
}

impl<T> DerefMut for DynVec<T>
where
    T: 'static + ?Sized + DynVecOps,
{
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.inst
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    #[should_panic(expected = "Type mismatch")]
    fn test_dynptr_cast_checked_panic_on_wrong_type() {
        let value = 15;
        let ptr = DynPtr::new(&value as *const i32);

        ptr.cast_checked::<usize>();
    }

    #[test]
    #[should_panic(expected = "Type mismatch")]
    fn test_dynptr_cast_checked_raw_panic_on_wrong_type() {
        let value = 15;
        let ptr = DynPtr::new(&value as *const i32);

        ptr.cast_checked_raw::<usize>();
    }

    #[test]
    fn test_vec_pool() {
        let mut pool: VecPool<i32> = VecPool::new();

        // Add some items to the pool.
        assert_eq!(pool.push(1), 0);
        assert_eq!(pool.push(2), 1);
        assert_eq!(pool.push(3), 2);

        // Reclaim a bunch of items.
        pool.reclaim(0);
        pool.reclaim(1);

        // Adding more items will fill up the holes first.
        assert_eq!(pool.push(3), 1);
        assert_eq!(pool.push(3), 0);
    }

    #[test]
    fn test_slot_pool() {
        let mut pool: SlotPool<i32> = SlotPool::new();

        // Add some items to the pool.
        assert_eq!(pool.push(1), 0);
        assert_eq!(pool.push(2), 1);
        assert_eq!(pool.push(3), 2);

        // Reclaim a bunch of items.
        pool.reclaim(0);
        pool.reclaim(1);

        // Adding more items will fill up the holes first.
        assert_eq!(pool.push(3), 1);

        // Retrieving a reclaimed slot yields `None`
        assert!(pool.get(0).is_none());

        // Retrieving out of bounds indices yields `None`
        assert!(pool.get(10).is_none());
    }

    #[test]
    fn test_slot_pool_peek_index() {
        let mut pool: SlotPool<i32> = SlotPool::new();

        // Add some items to the pool.
        assert_eq!(pool.push(1), 0);
        assert_eq!(pool.push(2), 1);

        // The next available index is at the tail
        assert_eq!(pool.peek_index(), 2);

        // Reclaim the head
        pool.reclaim(0);

        // Now, the next available index is at the head
        assert_eq!(pool.peek_index(), 0);
    }
}
