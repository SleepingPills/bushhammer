use crate::net::chunk::Chunk;
use crate::net::chunkpool::ChunkPool;
use std::collections::VecDeque;
use std::io;

/// An dynamically sized and double ended and buffered FIFO byte queue. Data is appended at the
/// head, and read from the tail.
pub struct Buffer {
    chunks: VecDeque<Chunk>,
}

impl Buffer {
    #[inline]
    pub fn new(pool: &mut ChunkPool) -> Buffer {
        let mut chunks = VecDeque::new();
        chunks.push_back(pool.alloc());
        Buffer { chunks }
    }

    /// Write the data from the buffer to the supplied writer. Returns Ok(()) in case all
    /// the data is written out, or the next write would block.
    pub fn egress<W: io::Write>(&mut self, writer: &mut W, pool: &mut ChunkPool) -> io::Result<()> {
        loop {
            // Consume chunks from the buffer until there is only one remaining
            match self.write(writer) {
                Ok(_) => {
                    if self.chunks.len() > 1 {
                        pool.reclaim(self.chunks.pop_front().unwrap());
                    } else {
                        // All data has been exhausted, nothing more to write.
                        return Ok(());
                    }
                }
                Err(e) => {
                    if e.kind() == io::ErrorKind::WouldBlock {
                        return Ok(());
                    } else {
                        return Err(e);
                    }
                }
            }
        }
    }

    /// Read the data from the reader into the buffer. Returns Ok(()) in case all the available
    /// data is read out and the next read would block.
    pub fn ingress<R: io::Read>(&mut self, reader: &mut R, pool: &mut ChunkPool) -> io::Result<()> {
        loop {
            // Keep adding chunks as long as there is data coming in or there is an error
            match self.read(reader) {
                Ok(_) => self.chunks.push_back(pool.alloc()),
                Err(e) => {
                    if e.kind() == io::ErrorKind::WouldBlock {
                        return Ok(());
                    } else {
                        return Err(e);
                    }
                }
            }
        }
    }

    #[inline]
    fn write<W: io::Write>(&mut self, writer: &mut W) -> io::Result<()> {
        let chunk = self.chunks.front_mut().unwrap();

        // Write to the writer as long as it is accepted or there is data in the chunk.
        loop {
            let write_count = writer.write(chunk.readable_slice())?;

            if write_count == 0 && chunk.remaining_data() > 0 {
                // No data written to the writer - operation should block but didn't
                return Err(io::ErrorKind::WouldBlock.into());
            }

            chunk.advance(write_count);

            if chunk.remaining_data() == 0 {
                return Ok(());
            }
        }
    }

    #[inline]
    fn read<R: io::Read>(&mut self, reader: &mut R) -> io::Result<()> {
        let chunk = self.chunks.back_mut().unwrap();

        // Read from the reader as long as there is data to read or the chunk has capacity.
        loop {
            let read_count = reader.read(chunk.writeable_slice())?;

            // No data read from the reader - operation should block but didn't
            if read_count == 0 && chunk.capacity() > 0 {
                return Err(io::ErrorKind::WouldBlock.into());
            }

            chunk.expand(read_count);

            if chunk.capacity() == 0 {
                return Ok(());
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::net::chunk::CHUNK_SIZE;
    use std::cmp::min;
    use std::io::Cursor;

    struct MockChannel {
        data: Vec<u8>,
        cursor: usize,
        chunk: usize,
        max_size: usize,
    }

    impl MockChannel {
        pub fn new(data: Vec<u8>, chunk: usize, max_size: usize) -> MockChannel {
            MockChannel {
                data,
                cursor: 0,
                chunk,
                max_size,
            }
        }

        pub fn clear(&mut self) {
            self.data.clear();
            self.cursor = 0;
        }
    }

    impl io::Read for MockChannel {
        fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
            if self.cursor == self.data.len() {
                return Err(io::ErrorKind::WouldBlock.into());
            }

            let offset = min(self.chunk, buf.len());
            buf[0..offset].copy_from_slice(&self.data[self.cursor..(self.cursor + offset)]);
            self.cursor += offset;
            Ok(offset)
        }
    }

    impl io::Write for MockChannel {
        fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
            if self.data.len() == self.max_size {
                return Err(io::ErrorKind::WouldBlock.into());
            }

            let count = min(self.chunk, buf.len());
            self.data.extend(&buf[0..count]);
            Ok(count)
        }

        fn flush(&mut self) -> io::Result<()> {
            unimplemented!()
        }
    }

    #[test]
    fn test_roundtrip() {
        let mock_data: Vec<_> = (0..(CHUNK_SIZE * 3)).map(|item| item as u8).collect();
        let mut channel = MockChannel::new(mock_data.clone(), 500, mock_data.len());

        let mut pool = ChunkPool::new();
        let mut buffer = Buffer::new(&mut pool);

        buffer.ingress(&mut channel, &mut pool).unwrap();

        channel.clear();

        assert_eq!(buffer.chunks.len(), 4);
        assert_eq!(buffer.chunks[0].readable_slice(), &mock_data[0..CHUNK_SIZE]);
        assert_eq!(buffer.chunks[1].readable_slice(), &mock_data[CHUNK_SIZE..CHUNK_SIZE * 2]);
        assert_eq!(buffer.chunks[2].readable_slice(), &mock_data[CHUNK_SIZE * 2..CHUNK_SIZE * 3]);

        buffer.egress(&mut channel, &mut pool).unwrap();

        assert_eq!(buffer.chunks.len(), 1);
        assert_eq!(buffer.chunks[0].capacity(), CHUNK_SIZE);
        assert_eq!(buffer.chunks[0].remaining_data(), 0);

        assert_eq!(channel.data[..], mock_data[..]);
    }

    #[test]
    fn test_no_err() {
        let mut cursor = Cursor::new(vec![1, 2, 3]);

        let mut pool = ChunkPool::new();
        let mut buffer = Buffer::new(&mut pool);

        buffer.ingress(&mut cursor, &mut pool).unwrap();

        assert_eq!(buffer.chunks.len(), 1);
        assert_eq!(buffer.chunks[0].readable_slice(), &vec![1, 2, 3][..]);

        let mut cursor = Cursor::new(Vec::<u8>::new());

        buffer.egress(&mut cursor, &mut pool).unwrap();

        assert_eq!(buffer.chunks.len(), 1);
        assert_eq!(buffer.chunks[0].readable_slice(), &Vec::<u8>::new()[..]);

        assert_eq!(&cursor.get_ref()[..], &vec![1, 2, 3][..]);
    }
}
