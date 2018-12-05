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

#[cfg(test)]
mod tests {
    use super::*;

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
