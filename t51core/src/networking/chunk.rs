use std::ops::{Deref, DerefMut};

const CHUNK_SIZE: usize = 8192;

/// A linear byte memory pool. Consuming data from the chunk will advance the start cursor, while
/// writing to the chunk will advance the end cursor. When the start cursor reaches the same
/// position as the end cursor, the chunk is assumed to be fully consumed (and thus empty).
pub struct Chunk {
    data: Box<[u8; CHUNK_SIZE]>,
    start: usize,
    end: usize,
}

impl Chunk {
    #[inline]
    pub fn new() -> Chunk {
        Chunk {
            data: Box::new([0; CHUNK_SIZE]),
            start: 0,
            end: 0,
        }
    }

    /// Free capacity in the chunk.
    #[inline]
    pub fn capacity(&self) -> usize {
        CHUNK_SIZE - self.end
    }

    /// Remaining data in the chunk
    #[inline]
    pub fn remaining_data(&self) -> usize {
        self.end - self.start
    }

    /// Read data from the chunk and advance the start cursor
    #[inline]
    pub fn read(&mut self, count: usize) -> &[u8] {
        let orig_start = self.start;
        let offset = self.start + count;

        self.validate_advance(count);
        self.start += count;
        self.check_clear();

        &self.data[orig_start..offset]
    }

    /// Write data to the chunk and advance the end cursor
    #[inline]
    pub fn write(&mut self, slice: &[u8]) {
        self.data[self.end..CHUNK_SIZE].copy_from_slice(slice);
        self.end += slice.len();
    }

    /// Advance the start cursor, as if a read has happened
    #[inline]
    pub fn advance(&mut self, count: usize) -> usize {
        let orig_start = self.start;
        self.validate_advance(count);
        self.start += count;
        self.check_clear();
        orig_start
    }

    #[inline]
    fn check_clear(&mut self) {
        // Clear the buffer in case we advance to the end
        if self.start == self.end {
            self.start = 0;
            self.end = 0;
        }
    }

    #[inline]
    fn validate_advance(&self, count: usize) {
        if self.start + count > self.end {
            panic!("Attempted to advance past buffer edge.")
        }
    }
}

/// The chunk immutably derefs to the available data slice
impl Deref for Chunk {
    type Target = [u8];

    fn deref(&self) -> &[u8] {
        &self.data[self.start..self.end]
    }
}

/// The chunk mutably derefs to the available capacity slice
impl DerefMut for Chunk {
    fn deref_mut(&mut self) -> &mut [u8] {
        &mut self.data[self.end..CHUNK_SIZE]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_new_pool() {}

    fn test_capacity() {}

    fn test_remaining_data() {}

    fn test_read() {}

    fn test_read_past_end_fails() {}

    fn test_write() {}

    fn test_write_past_capacity_fails() {}

    fn test_advance() {}

    fn test_deref_immut() {}

    fn test_deref_mut() {}
}
