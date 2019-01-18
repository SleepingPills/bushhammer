use slice_deque::SliceDeque;
use std::io;

type ByteDeque = SliceDeque<u8>;

// Buffer size set to be a multiple of the
const BUF_SIZE_INCREMENT: usize = 65536;

/// A dynamically sized and double ended and buffered FIFO byte queue. Data is appended at the
/// head, and read from the tail.
pub struct Buffer {
    data: ByteDeque,
    size: usize,
}

impl Buffer {
    #[inline]
    pub fn new(size: usize) -> Buffer {
        if size % BUF_SIZE_INCREMENT != 0 {
            panic!(
                "Buffer size must be divisible by {}, got {}",
                BUF_SIZE_INCREMENT, size
            );
        }

        let mut data = ByteDeque::new();
        data.reserve(size);
        Buffer { data, size }
    }

    /// The number of bytes in the buffer.
    #[inline]
    pub fn len(&self) -> usize {
        self.data.len()
    }

    /// Returns true in case the buffer is empty, false otherwise.
    #[inline]
    pub fn is_empty(&self) -> bool {
        self.data.is_empty()
    }

    /// Remaining free capacity in the buffer.
    #[inline]
    pub fn free_capacity(&self) -> usize {
        self.data.capacity() - self.data.len()
    }

    /// Advance the head.
    #[inline]
    pub fn move_head(&mut self, count: usize) {
        unsafe { self.data.move_head(count as isize) }
    }

    /// Advance the tail.
    #[inline]
    pub fn move_tail(&mut self, count: usize) {
        unsafe { self.data.move_tail(count as isize) }
    }

    /// Slice containing data.
    #[inline]
    pub fn read_slice(&self) -> &[u8] {
        self.data.as_slice()
    }

    #[inline]
    pub fn clear(&mut self) {
        unsafe { self.data.move_head(self.len() as isize) };
    }

    /// Slice containing free capacity to be written.
    #[inline]
    pub fn write_slice(&mut self) -> &mut [u8] {
        unsafe { self.data.tail_head_slice() }
    }

    /// Write the contents of the buffer to the supplied writer, advancing the read offset.
    #[inline]
    pub fn egress<W: io::Write>(&mut self, mut writer: W) -> io::Result<usize> {
        let orig_len = self.data.len();

        while self.data.len() > 0 {
            let write_count = writer.write(&self.data)?;

            if write_count == 0 {
                return Err(io::ErrorKind::WriteZero.into());
            }

            self.move_head(write_count);
        }

        Ok(orig_len - self.data.len())
    }

    /// Read in data from the supplied reader to the buffer.
    #[inline]
    pub fn ingress<R: io::Read>(&mut self, mut reader: R) -> io::Result<usize> {
        let orig_capacity = self.free_capacity();

        while self.data.len() < self.size {
            unsafe {
                let read_count = reader.read(self.data.tail_head_slice())?;

                if read_count == 0 {
                    return Ok(orig_capacity - self.free_capacity());
                }

                self.move_tail(read_count);
            }
        }

        Err(io::Error::new(io::ErrorKind::Other, "Buffer overrun"))
    }

    /// Mutable slice containing data.
    #[allow(dead_code)]
    #[inline]
    pub(crate) fn data_slice(&mut self) -> &mut [u8] {
        self.data.as_mut_slice()
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

            let offset = min(min(self.chunk, buf.len()), self.data.len() - self.cursor);
            buf[..offset].copy_from_slice(&self.data[self.cursor..(self.cursor + offset)]);
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
            self.data.extend(&buf[..count]);

            Ok(count)
        }

        fn flush(&mut self) -> io::Result<()> {
            unimplemented!()
        }
    }

    #[test]
    fn test_roundtrip() {
        let mock_data: Vec<_> = (0..BUF_SIZE_INCREMENT / 2).map(|item| item as u8).collect();
        let mut channel = MockChannel::new(mock_data.clone(), 500, mock_data.len());

        let mut buffer = Buffer::new(BUF_SIZE_INCREMENT);

        let result = buffer.ingress(&mut channel);

        assert!(result.is_err());
        assert_eq!(result.err().unwrap().kind(), io::ErrorKind::WouldBlock);
        assert_eq!(buffer.data.len(), mock_data.len());
        assert_eq!(buffer.data.as_slice(), &mock_data[..]);

        channel.clear();
        let count = buffer.egress(&mut channel).unwrap();

        assert_eq!(count, mock_data.len());
        assert_eq!(buffer.data.len(), 0);
        assert_eq!(channel.data[..], mock_data[..]);
    }

    #[test]
    fn test_egress_error_on_zero_write() {
        let mut zero_vec = vec![];

        let mut buffer = Buffer::new(BUF_SIZE_INCREMENT);

        // The buffer has to have at least some data to trigger the zero write error
        buffer.data.push_back(1);

        let result = buffer.egress(&mut zero_vec[..]);

        assert!(result.is_err());
        assert_eq!(result.err().unwrap().kind(), io::ErrorKind::WriteZero);
    }

    #[test]
    fn test_ingress_buffer_overrun() {
        let mock_data: Vec<_> = (0..BUF_SIZE_INCREMENT * 2).map(|item| item as u8).collect();

        let mut buffer = Buffer::new(BUF_SIZE_INCREMENT);

        let result = buffer.ingress(&mock_data[..]);

        assert!(result.is_err());

        let err = result.err().unwrap();

        assert_eq!(err.kind(), io::ErrorKind::Other);
        assert_eq!(err.to_string(), "Buffer overrun")
    }

    #[test]
    fn test_no_err() {
        let mut cursor = Cursor::new(vec![1, 2, 3]);
        let mut buffer = Buffer::new(BUF_SIZE_INCREMENT);

        buffer.ingress(&mut cursor).unwrap();

        assert_eq!(buffer.data.as_slice(), &[1, 2, 3]);

        let mut cursor = Cursor::new(Vec::<u8>::new());

        buffer.egress(&mut cursor).unwrap();

        assert_eq!(buffer.data.as_slice(), &Vec::<u8>::new()[..]);

        assert_eq!(&cursor.get_ref()[..], &[1, 2, 3]);
    }

    #[test]
    #[should_panic(expected = "Buffer size must be divisible by 65536, got 100000")]
    fn test_fail_on_incorrect_increment() {
        let _ = Buffer::new(100000);
    }
}
