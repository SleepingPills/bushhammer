use crate::net::buffer::Buffer;
use crate::net::chunkpool::ChunkPool;
use crate::net::frame::Frame;
use crate::net::frame::Header;
use bincode;
use std::io;
use std::net::TcpStream;

pub enum Error {
    NeedMore,
    CorruptData,
    Network(io::Error),
}

impl From<bincode::Error> for Error {
    fn from(_: Box<bincode::ErrorKind>) -> Self {
        Error::CorruptData
    }
}

pub type TxResult<T> = Result<T, Error>;

pub struct Channel {
    sequence: u64,
    stream: TcpStream,
    crypto_buffer: Vec<u8>,
    read_buffer: Buffer,
    write_buffer: Buffer,
    frame: Frame,
}

impl Channel {
    pub fn recieve(&mut self) -> TxResult<()> {
        match self.read_buffer.ingress(&mut self.stream) {
            Err(e) => Err(Error::Network(e)),
            _ => Ok(()),
        }
    }

    pub fn read(&mut self) -> TxResult<&Frame> {
        let header: Header = bincode::deserialize_from(&mut self.read_buffer)?;
        unimplemented!()
    }
}

pub trait AwaitToken {
    fn read_token(&mut self, secret_key: &[u8; 32]) -> TxResult<&Frame>;
}

pub trait Challenge {
    fn read_challenge(&mut self) -> TxResult<&Frame>;
}

pub trait Connected {
    fn read_frame(&mut self) -> TxResult<&Frame>;
}
