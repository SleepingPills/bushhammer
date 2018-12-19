use crate::networking::chunk::Chunk;
use crate::networking::chunkpool::ChunkPool;
use std::io;

/// An dynamically sized and double ended and buffered FIFO byte queue. Data is appended at the
/// head, and read from the tail.
pub struct Buffer {
    chunks: Vec<Chunk>,
}

impl Buffer {
    #[inline]
    pub fn new(pool: &mut ChunkPool) -> Buffer {
        Buffer {
            chunks: vec![pool.alloc()],
        }
    }

    /// Write the data from the buffer to the supplied writer. Returns Ok(()) in case all
    /// the data is written out, or the next write would block.
    pub fn egress<W: io::Write>(&mut self, writer: &mut W, pool: &mut ChunkPool) -> io::Result<()> {
        loop {
            // Consume chunks from the buffer until there is only one remaining
            match self.write(writer) {
                Ok(_) => {
                    if self.chunks.len() > 1 {
                        pool.reclaim(self.chunks.swap_remove(0));
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
                Ok(_) => self.chunks.push(pool.alloc()),
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
    pub fn write<W: io::Write>(&mut self, writer: &mut W) -> io::Result<()> {
        let chunk = unsafe { self.chunks.get_unchecked_mut(0) };

        // Write to the writer as long as it is accepted or there is data in the chunk.
        loop {
            let write_count = writer.write(chunk)?;
            chunk.advance(write_count);

            if chunk.remaining_data() == 0 {
                return Ok(());
            }
        }
    }

    #[inline]
    pub fn read<R: io::Read>(&mut self, reader: &mut R) -> io::Result<()> {
        let chunks_len = self.chunks.len();
        let chunk = unsafe { self.chunks.get_unchecked_mut(chunks_len - 1) };

        // Read from the reader as long as there is data to read or the chunk has capacity.
        loop {
            let read_count = reader.read(chunk)?;
            chunk.expand(read_count);

            if chunk.capacity() == 0 {
                return Ok(());
            }
        }
    }
}
