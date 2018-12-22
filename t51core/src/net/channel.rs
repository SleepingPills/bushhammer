use crate::net::buffer::Buffer;
use crate::net::chunkpool::ChunkPool;
use std::io;
use std::net::TcpStream;

pub struct Channel {
    sequence: u64,
    stream: TcpStream,
    crypto_buffer: Vec<u8>,
    read_buffer: Buffer,
    write_buffer: Buffer,
}

impl Channel {
    fn recieve(&mut self, pool: &mut ChunkPool) -> io::Result<()> {
        self.read_buffer.ingress(&mut self.stream, pool)
    }
}

pub trait AwaitToken {}

pub trait Challenge {}

pub trait Connected {}
