use std::io;
use bincode;

pub enum Error {
    Data(Box<bincode::ErrorKind>),
    Io(io::Error),
}

impl From<io::Error> for Error {
    fn from(io_error: io::Error) -> Self {
        Error::Io(io_error)
    }
}

impl From<bincode::Error> for Error {
    fn from(bincode_error: Box<bincode::ErrorKind>) -> Self {
        Error::Data(bincode_error)
    }
}

pub type Result<T> = ::std::result::Result<T, Error>;