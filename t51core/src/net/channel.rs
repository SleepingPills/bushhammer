use crate::net::buffer::Buffer;
use crate::net::chunkpool::ChunkPool;
use crate::net::frame::Frame;
use bincode;
use std::io;
use std::net::TcpStream;

pub enum Error {
    NeedMore,
    DataCorrupted,
    Network(io::Error)
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
    fn recieve(&mut self, pool: &mut ChunkPool) -> TxResult<()> {
        match self.read_buffer.ingress(&mut self.stream, pool) {
            Err(e) => Err(Error::Network(e)),
            _ => Ok(())
        }
    }

    fn read(&mut self, pool: &mut ChunkPool) -> TxResult<&Frame> {
//        let header = bincode::deserialize_from()
        unimplemented!()
    }
}

pub trait AwaitToken {}

pub trait Challenge {}

pub trait Connected {}
