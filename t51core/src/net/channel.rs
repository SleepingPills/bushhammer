use crate::net::buffer::{Buffer, BUF_SIZE};
use crate::net::crypto;
use crate::net::error::{Error, Result};
use crate::net::frame::{ConnectionToken, Header};
use crate::net::shared::{current_timestamp, ClientId};
use bincode;
use std::io;
use std::net::TcpStream;

pub struct Channel {
    // Tcp Stream
    stream: TcpStream,

    // Crypto
    version: [u8; 16],
    protocol: u16,
    sequence: u64,

    additional_data: [u8; 19],
    server_key: [u8; crypto::KEY_SIZE],
    client_key: [u8; crypto::KEY_SIZE],

    // Channel State
    read_buffer: Buffer,
    write_buffer: Buffer,
    payload: [u8; BUF_SIZE],
}

impl Channel {
    #[inline]
    pub fn new(stream: TcpStream, version: [u8; 16], protocol: u16) -> Channel {
        let mut server_key = [0u8; crypto::KEY_SIZE];
        let mut client_key = [0u8; crypto::KEY_SIZE];

        crypto::random_bytes(&mut server_key);
        crypto::random_bytes(&mut client_key);

        // TODO: Prepopulate with version, protocol and class
        let mut additional_data = [0u8; 19];

        Channel {
            stream,
            version,
            protocol,
            sequence: 0,
            server_key,
            client_key,
            read_buffer: Buffer::new(),
            write_buffer: Buffer::new(),
            payload: vec![],
        }
    }

    #[inline]
    pub fn send(&mut self) -> Result<usize> {
        self.write_buffer.egress(&mut self.stream).map_err(Into::into)
    }

    #[inline]
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
    fn read_frame(&mut self) -> Result<&[u8]>;
}

impl Connected for Channel {
    fn read_frame(&mut self) -> Result<&[u8]> {
        let stream = self.read_buffer.read_slice();
        let header = Header::deserialize(stream)?;
        let payload_size = header.size as usize;

        if stream.len() < payload_size {
            return Err(Error::Io(io::ErrorKind::WouldBlock.into()));
        }

        let decrypted_size = payload_size - crypto::MAC_SIZE;

        crypto::decrypt(
            &mut self.payload[..decrypted_size],
            &stream[..payload_size],
            &self.additional_data,
            header.sequence,
            &self.server_key,
        );

        Ok(&self.payload[..decrypted_size])
    }
}
