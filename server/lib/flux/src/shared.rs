use std::io;
use std::net;

pub const PROTOCOL_ID: u16 = 0x0a55;
pub const VERSION_ID: [u8; 16] = [0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0];

pub type UserId = u64;

pub type NetworkResult<T> = Result<T, NetworkError>;

#[derive(Debug, Eq, PartialEq)]
pub enum NetworkError {
    Wait,
    Fatal(ErrorType),
}

#[derive(Debug, Eq, PartialEq)]
pub enum ErrorType {
    Expired,
    Duplicate,
    AlreadyConnected,
    PayloadTooLarge,
    EmptyPayload,
    IncorrectCategory,
    ProtocolMismatch,
    VersionMismatch,
    SequenceMismatch,
    Serialization,
    Crypto,
    AddrParse,
    Io(io::ErrorKind),
}

impl From<io::Error> for NetworkError {
    #[inline]
    fn from(io_error: io::Error) -> Self {
        match io_error.kind() {
            io::ErrorKind::WouldBlock => NetworkError::Wait,
            kind => NetworkError::Fatal(ErrorType::Io(kind)),
        }
    }
}

impl From<net::AddrParseError> for NetworkError {
    #[inline]
    fn from(_: net::AddrParseError) -> Self {
        NetworkError::Fatal(ErrorType::AddrParse)
    }
}

pub trait ErrorUtils {
    fn has_failed(&self) -> bool;
}

impl<T> ErrorUtils for NetworkResult<T> {
    fn has_failed(&self) -> bool {
        match self {
            Ok(_) => false,
            Err(NetworkError::Wait) => false,
            _ => true,
        }
    }
}

/// Augmented `io::Write` that is aware of the amount of remaining free capacity in the destination.
pub trait SizedWrite: io::Write {
    /// Remaining free capacity in the destination.
    fn free_capacity(&self) -> usize;
}

/// Augmented `io::Read` that is aware of the amount of remaining data in the source.
pub trait SizedRead: io::Read {
    /// Remaining data in the source.
    fn remaining_data(&self) -> usize;
}

impl SizedWrite for io::Cursor<&mut [u8]> {
    #[inline]
    fn free_capacity(&self) -> usize {
        self.get_ref().len() - self.position() as usize
    }
}

impl SizedRead for io::Cursor<&[u8]> {
    #[inline]
    fn remaining_data(&self) -> usize {
        self.get_ref().len() - self.position() as usize
    }
}

/// Trait for manually serialized objects. Implementors must take care to validate the remaining
/// free capacity in the stream upfront and only write into it if all the content they wish to
/// write can be written.
///
/// Should return `Error::Wait` in case there is not enough capacity in the stream.
pub trait Serialize {
    fn serialize<W: SizedWrite>(&self, stream: &mut W) -> NetworkResult<()>;
}

/// Trait for manually deserialized objects.
pub trait Deserialize: Sized {
    fn deserialize<R: SizedRead>(stream: &mut R) -> NetworkResult<Self>;
}

/// Batched payload messages for efficient serialization/deserialization.
pub struct PayloadBatch<P> {
    data: Vec<P>,
}

impl<P> PayloadBatch<P> {
    /// Creates a new `PayloadBatch` instance.
    #[inline]
    pub fn new() -> PayloadBatch<P> {
        PayloadBatch { data: Vec::new() }
    }

    /// Returns the number of payload messages in the batch.
    #[inline]
    pub fn len(&self) -> usize {
        self.data.len()
    }
}

impl<P: Serialize> PayloadBatch<P> {
    /// Push a new payload message on the batch.
    #[inline]
    pub fn push(&mut self, payload: P) {
        self.data.push(payload)
    }

    /// Drain payload messages from the batch.
    #[inline]
    pub fn drain(&mut self) -> impl Iterator<Item = P> + '_ {
        self.data.drain(..)
    }

    /// Write as many payload messages as possible to the destination stream.
    #[inline]
    pub fn write<W: SizedWrite>(&mut self, stream: &mut W) -> NetworkResult<()> {
        let mut remaining = self.data.len();

        for payload in self.data.iter_mut() {
            match payload.serialize(stream) {
                Ok(_) => remaining -= 1,
                Err(NetworkError::Wait) => break,
                Err(error) => return Err(error),
            }
        }

        // Bail out in case nothing could be written into the stream
        if remaining == self.data.len() {
            return Err(NetworkError::Wait);
        }

        self.data.truncate(remaining);
        Ok(())
    }
}

impl<P: Deserialize> PayloadBatch<P> {
    /// Read as many messages as possible form the source stream into the current batch.
    #[inline]
    pub fn read<R: SizedRead>(&mut self, stream: &mut R) -> NetworkResult<()> {
        while stream.remaining_data() > 0 {
            self.data.push(P::deserialize(stream)?)
        }

        Ok(())
    }
}
