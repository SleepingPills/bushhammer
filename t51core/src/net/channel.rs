use crate::net::buffer::Buffer;
use crate::net::error::{Error, Result};
use crate::net::frame::{ConnectionToken, Header};
use crate::net::shared::{ClientId, current_timestamp};
use bincode;
use std::io;
use std::net::TcpStream;

pub struct Channel {
    // Tcp Stream
    stream: TcpStream,

    // Crypto
    version: [u8; 16],
    protocol: u64,
    sequence: u64,

    // TODO: The constructor will fill these with random bytes, ensuring that usage before
    // connecting will fail the decryption.
    server_key: [u8; 32],
    client_key: [u8; 32],

    // Channel State
    read_buffer: Buffer,
    write_buffer: Buffer,
    payload: Vec<u8>,
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
}

impl AwaitToken for Channel {
    fn read_connection_token(&mut self, secret_key: &[u8; 32]) -> Result<ClientId> {
        let token = ConnectionToken::deserialize(&mut self.read_buffer, secret_key)?;

        if token.expires < current_timestamp() {
            return Err(Error::Expired);
        }

        if token.protocol != self.protocol {
            return Err(Error::ProtocolMismatch);
        }

        if token.version != self.version {
            return Err(Error::VersionMismatch);
        }

        self.server_key = token.data.server_key;
        self.client_key = token.data.client_key;

        Ok(token.data.client_id)
    }
}

pub trait Connected {
    fn read_frame(&mut self) -> Result<&Vec<u8>>;
}

impl Connected for Channel {
    fn read_frame(&mut self) -> Result<&Vec<u8>> {
        //        let header: Header = bincode::deserialize_from(&mut self.read_buffer)?;
        unimplemented!()
    }
}
