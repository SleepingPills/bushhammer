use crate::net::buffer::Buffer;
use crate::net::shared::ClientId;
use crate::net::error::{Error, Result};
//use crate::net::frame::{Frame, PayloadHeader, PrivateData};
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
//    frame: Frame,
}

impl Channel {
    pub fn send(&mut self) -> Result<usize> {
        self.write_buffer.egress(&mut self.stream).map_err(Into::into)
    }

    pub fn recieve(&mut self) -> Result<usize> {
        self.read_buffer.ingress(&mut self.stream).map_err(Into::into)
    }
}

pub trait AwaitToken {
    /// Reads the connection token off the channel, parses the contents and returns the client id.
    fn read_connection_token(&mut self, secret_key: &[u8; 32]) -> Result<ClientId>;

    /// Writes the connection challenge to the channel.
    fn write_connection_challenge(&mut self) -> Result<()>;
}

impl AwaitToken for Channel {
    fn read_connection_token(&mut self, secret_key: &[u8; 32]) -> Result<ClientId> {
//        let token_header: ConnectionHeader = bincode::deserialize_from(&mut self.read_buffer)?;

        Ok(100)
    }

    fn write_connection_challenge(&mut self) -> Result<()> {
        unimplemented!()
    }
}

//pub trait Connected {
//    fn read_frame(&mut self) -> Result<&Frame>;
//}
//
//impl Connected for Channel {
//    fn read_frame(&mut self) -> Result<&Frame> {
//        //        let header: Header = bincode::deserialize_from(&mut self.read_buffer)?;
//        unimplemented!()
//    }
//}
