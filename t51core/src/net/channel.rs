use crate::net::buffer::Buffer;
use crate::net::error::{Error, TxResult};
use crate::net::frame::{ConnectionHeader, PayloadHeader, PrivateData, Frame};
use bincode;
use std::io;
use std::net::TcpStream;

pub struct Channel {
    // Tcp Stream
    stream: TcpStream,

    // Crypto
    server_key: [u8; 32],
    client_key: [u8; 32],
    crypto_buffer: Vec<u8>,

    // Channel State
    sequence: u64,
    read_buffer: Buffer,
    write_buffer: Buffer,
    frame: Frame,
}

impl Channel {
    pub fn send(&mut self) -> io::Result<usize> {
        self.write_buffer.egress(&mut self.stream)
    }

    pub fn recieve(&mut self) -> io::Result<usize> {
        self.read_buffer.ingress(&mut self.stream)
    }
}

pub trait AwaitToken {
    /// Reads the connection token off the channel, parses the contents and returns the client id.
    fn read_connection_token(&mut self, secret_key: &[u8; 32]) -> io::Result<u64>;

    /// Writes the connection challenge to the channel.
    fn write_connection_challenge(&mut self) -> io::Result<()>;
}

impl AwaitToken for Channel {
    fn read_connection_token(&mut self, secret_key: &[u8; 32]) -> io::Result<u64> {
//        let token_header: ConnectionHeader = match bincode::deserialize_from(&mut self.read_buffer) {
//            Ok(header) => header,
//            Err(bincode::ErrorKind::) => return Err(io::ErrorKind::)
//        };

        Ok(100)
    }

    fn write_connection_challenge(&mut self) -> io::Result<()> {
        unimplemented!()
    }
}

pub trait Challenge {
    fn read_challenge_response(&mut self) -> io::Result<&Frame>;
}

pub trait Connected {
    fn read_frame(&mut self) -> io::Result<&Frame>;
}

impl Connected for Channel {
    fn read_frame(&mut self) -> io::Result<&Frame> {
        //        let header: Header = bincode::deserialize_from(&mut self.read_buffer)?;
        unimplemented!()
    }
}
