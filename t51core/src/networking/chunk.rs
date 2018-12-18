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
        self.data[self.end..(self.end + slice.len())].copy_from_slice(slice);
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
            panic!("Attempted to advance past chunk edge.")
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

    #[test]
    fn test_new_chunk() {
        let chunk = Chunk::new();

        assert_eq!(chunk.data.len(), CHUNK_SIZE);
        assert_eq!(chunk.start, 0);
        assert_eq!(chunk.end, 0);
    }

    #[test]
    fn test_capacity() {
        let mut chunk = Chunk::new();

        assert_eq!(chunk.capacity(), CHUNK_SIZE);

        chunk.end = 1000;

        assert_eq!(chunk.capacity(), CHUNK_SIZE - 1000)
    }

    #[test]
    fn test_remaining_data() {
        let mut chunk = Chunk::new();

        assert_eq!(chunk.remaining_data(), 0);

        chunk.end = 1000;

        assert_eq!(chunk.remaining_data(), 1000);
    }

    #[test]
    fn test_read() {
        let mut chunk = Chunk::new();

        chunk.data[..4].copy_from_slice(&vec![1, 2, 3, 4]);
        chunk.end = 4;

        assert_eq!(chunk.read(2), vec![1u8, 2u8].as_slice());
        assert_eq!(chunk.start, 2);
        assert_eq!(chunk.end, 4);

        // Reading to the end resets the chunk to the empty state
        assert_eq!(chunk.read(2), vec![3u8, 4u8].as_slice());
        assert_eq!(chunk.start, 0);
        assert_eq!(chunk.end, 0);
    }

    #[test]
    #[should_panic(expected = "Attempted to advance past chunk edge.")]
    fn test_read_past_end_fails() {
        let mut chunk = Chunk::new();

        chunk.data[..4].copy_from_slice(&vec![1, 2, 3, 4]);
        chunk.end = 4;

        chunk.read(5);
    }

    #[test]
    fn test_write() {
        let mut chunk = Chunk::new();

        let items: Vec<u8> = (1..5).collect();

        chunk.write(&items);

        assert_eq!(&chunk.data[..4], items.as_slice());
        assert_eq!(chunk.start, 0);
        assert_eq!(chunk.end, 4);
    }

    #[test]
    #[should_panic(expected = "index 8193 out of range for slice of length 8192")]
    fn test_write_past_capacity_fails() {
        let mut chunk = Chunk::new();

        let items: Vec<u8> = (0..(CHUNK_SIZE + 1)).map(|item| item as u8).collect();

        chunk.write(&items);
    }

    #[test]
    fn test_advance() {
        let mut chunk = Chunk::new();

        chunk.end = 5;

        chunk.advance(2);
        assert_eq!(chunk.start, 2);
        assert_eq!(chunk.end, 5);

        chunk.advance(3);
        assert_eq!(chunk.start, 0);
        assert_eq!(chunk.end, 0);
    }

    #[test]
    fn test_deref_immut() {
        let mut chunk = Chunk::new();

        let chunk_slice: &[u8] = &chunk;
        assert_eq!(chunk_slice, Vec::<u8>::new().as_slice());

        let data = vec![1, 2, 3, 4];
        chunk.write(&data);

        let chunk_slice: &[u8] = &chunk;
        assert_eq!(chunk_slice, data.as_slice());
    }

    #[test]
    fn test_deref_mut() {
        let mut chunk = Chunk::new();

        let chunk_slice: &mut [u8] = &mut chunk;
        assert_eq!(chunk_slice.len(), CHUNK_SIZE);

        chunk.write(&vec![1, 2, 3, 4]);

        let chunk_slice: &mut [u8] = &mut chunk;
        assert_eq!(chunk_slice.len(), CHUNK_SIZE - 4);
    }
}
