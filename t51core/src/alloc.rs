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
    pub unsafe fn get_store_ptr(&self) -> *const T {
        self.store.as_ptr()
    }

    #[inline]
    pub unsafe fn get_store_mut_ptr(&mut self) -> *mut T {
        self.store.as_mut_ptr()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_rwcell() {
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
}
