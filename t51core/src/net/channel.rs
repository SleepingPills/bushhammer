use crate::net::buffer::Buffer;
use crate::net::error::{Error, TxResult};
use crate::net::frame::Frame;
use crate::net::frame::{ConnectionToken, PayloadHeader, PrivateData};
use bincode;
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
    pub fn send(&mut self) -> TxResult<()> {
        self.write_buffer.egress(&mut self.stream)?;
        Ok(())
    }

    pub fn recieve(&mut self) -> TxResult<()> {
        self.read_buffer.ingress(&mut self.stream)?;
        Ok(())
    }
}

pub trait AwaitToken {
    /// Reads the connection token off the channel, parses the contents and returns the client id.
    fn read_connection_token(&mut self, secret_key: &[u8; 32]) -> TxResult<u64>;

    /// Writes the connection challenge to the channel.
    fn write_connection_challenge(&mut self) -> TxResult<()>;
}

impl AwaitToken for Channel {
    fn read_connection_token(&mut self, secret_key: &[u8; 32]) -> TxResult<u64> {
        /*
        Additional data for decryption
        [version info] (16 bytes)       // "NETCODE 1.02" ASCII with null terminator.
        [protocol id] (uint64)          // 64 bit value unique to this particular game/application
        [expire timestamp] (uint64)     // 64 bit unix timestamp when this connect token expires
        */
        //        let token_header: ConnectionHeader = bincode::deserialize_from(&mut self.read_buffer)?;
        Ok(100)
    }

    fn write_connection_challenge(&mut self) -> TxResult<()> {
        unimplemented!()
    }
}

pub trait Challenge {
    fn read_challenge_response(&mut self) -> TxResult<&Frame>;
}

pub trait Connected {
    fn read_frame(&mut self) -> TxResult<&Frame>;
}

impl Connected for Channel {
    fn read_frame(&mut self) -> TxResult<&Frame> {
        //        let header: Header = bincode::deserialize_from(&mut self.read_buffer)?;
        unimplemented!()
    }
}
