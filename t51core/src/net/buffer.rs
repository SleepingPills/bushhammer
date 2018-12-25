use slice_deque::SliceDeque;
use std::cmp::min;
use std::io;
use std::ptr;

type ByteDeque = SliceDeque<u8>;
const BUF_SIZE: usize = 32768;

/// An dynamically sized and double ended and buffered FIFO byte queue. Data is appended at the
/// head, and read from the tail.
pub struct Buffer {
    offset: usize,
    data: ByteDeque,
}

impl Buffer {
    #[inline]
    pub fn new() -> Buffer {
        let mut data = ByteDeque::new();
        data.reserve(BUF_SIZE);
        Buffer { offset: 0, data }
    }

    /// Advance the cursor to the current read offset.
    #[inline]
    pub fn advance(&mut self) {
        unsafe { self.data.move_head(self.offset as isize) }
        self.offset = 0;
    }

    /// Roll back the read offset to the position last advanced to.
    #[inline]
    pub fn rollback(&mut self) {
        self.offset = 0;
    }

    /// Write the contents of the buffer to the supplied writer, advancing the read offset.
    pub fn egress<W: io::Write>(&mut self, mut writer: W) -> io::Result<usize> {
        let prev_offset = self.offset;

        while self.offset < self.data.len() {
            let write_count = writer.write(&self.data[self.offset..])?;

            if write_count == 0 {
                return match self.data.len() {
                    0 => Ok(self.offset - prev_offset),
                    _ => Err(io::ErrorKind::WriteZero.into()),
                };
            }

            self.offset += write_count;
        }

        Ok(self.offset - prev_offset)
    }

    /// Read in data from the supplied reader to the buffer.
    pub fn ingress<R: io::Read>(&mut self, mut reader: R) -> io::Result<usize> {
        let mut total_count = 0usize;

        loop {
            unsafe {
                let read_count = reader.read(self.data.tail_head_slice())?;

                if read_count == 0 {
                    return match self.data.len() < BUF_SIZE {
                        true => Ok(total_count),
                        _ => Err(io::Error::new(io::ErrorKind::Other, "Buffer overrun")),
                    };
                }

                self.data.move_tail(read_count as isize);
                total_count += read_count;
            }
        }
    }
}

impl io::Read for Buffer {
    #[inline]
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        // Count is the smaller of the buffer length and the remaining data.
        let count = min(self.data.len() - self.offset, buf.len());

        // Memcpy the data directly
        unsafe {
            ptr::copy_nonoverlapping(self.data.as_ptr().add(self.offset), buf.as_mut_ptr(), count);
        }

        // Bump the read offset
        self.offset += count;
        Ok(count)
    }

    #[inline]
    fn read_exact(&mut self, buf: &mut [u8]) -> io::Result<()> {
        // Bail out early in case there isn't enough data to fill the buffer
        if (self.offset + buf.len()) > self.data.len() {
            return Err(io::ErrorKind::UnexpectedEof.into());
        }

        unsafe {
            ptr::copy_nonoverlapping(self.data.as_ptr().add(self.offset), buf.as_mut_ptr(), buf.len());
        }

        // Bump the read offset
        self.offset += buf.len();
        Ok(())
    }
}

impl io::Write for Buffer {
    #[inline]
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        unsafe {
            let write_slice = self.data.tail_head_slice();
            let count = min(write_slice.len(), buf.len());

            // Write directly into the tail slice
            ptr::copy_nonoverlapping(buf.as_ptr(), write_slice.as_mut_ptr(), count);

            Ok(count)
        }
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
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
        let mock_data: Vec<_> = (0..BUF_SIZE).map(|item| item as u8).collect();
        let mut channel = MockChannel::new(mock_data.clone(), 500, mock_data.len());

        let mut buffer = Buffer::new();

        let result = buffer.ingress(&mut channel);

        assert!(result.is_err());
        assert_eq!(result.err().unwrap().kind(), io::ErrorKind::WouldBlock);
        assert_eq!(buffer.data.len(), BUF_SIZE);
        assert_eq!(buffer.data.as_slice(), &mock_data[..]);

        channel.clear();
        buffer.egress(&mut channel).unwrap();

        assert_eq!(buffer.offset, BUF_SIZE);
        assert_eq!(buffer.data.as_slice(), &mock_data[..]);

        buffer.advance();

        assert_eq!(buffer.data.len(), 0);
        assert_eq!(channel.data[..], mock_data[..]);
    }

    #[test]
    fn test_egress_error_on_zero_write() {

    }

    #[test]
    fn test_ingress_buffer_overrun() {

    }

    #[test]
    fn test_no_err() {
        let mut cursor = Cursor::new(vec![1, 2, 3]);
        let mut buffer = Buffer::new();

        buffer.ingress(&mut cursor).unwrap();

        assert_eq!(buffer.data.as_slice(), &vec![1, 2, 3][..]);

        let mut cursor = Cursor::new(Vec::<u8>::new());

        buffer.egress(&mut cursor).unwrap();

        assert_eq!(buffer.offset, 3);
        assert_eq!(buffer.data.as_slice(), &vec![1, 2, 3][..]);

        buffer.advance();

        assert_eq!(buffer.offset, 0);
        assert_eq!(buffer.data.as_slice(), &Vec::<u8>::new()[..]);

        assert_eq!(&cursor.get_ref()[..], &vec![1, 2, 3][..]);
    }

    #[test]
    fn test_read() {

    }

    #[test]
    fn test_read_exact() {

    }

    #[test]
    fn test_write() {

    }
}
