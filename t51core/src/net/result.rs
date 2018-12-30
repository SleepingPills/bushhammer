use std::io;

#[derive(Debug, Eq, PartialEq)]
pub enum Error {
    Expired,
    Duplicate,
    AlreadyConnected,
    PayloadTooLarge,
    Wait,
    IncorrectCategory,
    ProtocolMismatch,
    VersionMismatch,
    SequenceMismatch,
    Serialization,
    Crypto,
    Io(io::ErrorKind),
}

impl From<io::Error> for Error {
    fn from(io_error: io::Error) -> Self {
        Error::Io(io_error.kind())
    }
}

pub type Result<T> = ::std::result::Result<T, Error>;