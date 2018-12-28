use std::io;

pub enum Error {
    Expired,
    Duplicate,
    PayloadTooLarge,
    Wait,
    IncorrectCategory,
    ProtocolMismatch,
    VersionMismatch,
    SequenceMismatch,
    Serialization,
    Crypto,
    Io(io::Error),
}

impl From<io::Error> for Error {
    fn from(io_error: io::Error) -> Self {
        Error::Io(io_error)
    }
}

pub type Result<T> = ::std::result::Result<T, Error>;