use crate::net::buffer::Buffer;
use std::io;
use std::net::TcpStream;

pub struct Channel {
    stream: TcpStream,
    crypto_buffer: Vec<u8>,
    read_buffer: Buffer,
    write_buffer: Buffer,
}

impl Channel {
    fn recieve(&mut self) -> io::Result<()> {
        unimplemented!()
    }
}
