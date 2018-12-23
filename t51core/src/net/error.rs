use std::io;

pub enum Error {
    NeedMore,
    CorruptData,
    Network(io::Error),
}

impl From<io::Error> for Error {
    fn from(io_error: io::Error) -> Self {
        Error::Network(io_error)
    }
}

pub type TxResult<T> = Result<T, Error>;