use crate::net::buffer::{Buffer, BUF_SIZE};
use crate::net::crypto;
use crate::net::result::{Error, Result};
use crate::net::shared::Serializable;
use crate::net::shared::{current_timestamp, ClientId};
use byteorder::{BigEndian, LittleEndian, ReadBytesExt, WriteBytesExt};
use std::io::{Cursor, Read, Write};
use std::mem;
use std::net::TcpStream;

pub struct Channel {
    // Tcp Stream
    stream: TcpStream,

    // Validation
    version: [u8; 16],
    protocol: u16,

    // Sequence of packets recieved from the client
    client_sequence: u64,
    // Sequence of packets sent to the client
    server_sequence: u64,

    // Client2Server Key
    server_key: [u8; crypto::KEY_SIZE],
    // Server2Client Key
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

        // Prime the keys with random data, ensuring that communication can't happen before the
        // handshake process finishes.
        crypto::random_bytes(&mut server_key);
        crypto::random_bytes(&mut client_key);

        Channel {
            stream,
            version,
            protocol,
            client_sequence: 0,
            server_sequence: 0,
            server_key,
            client_key,
            read_buffer: Buffer::new(),
            write_buffer: Buffer::new(),
            payload: [0; BUF_SIZE],
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

    #[inline]
    fn additional_data(&self, category: Category) -> [u8; 19] {
        let mut additional_data = [0u8; 19];
        {
            let mut buf = &mut additional_data[..];
            buf.write_all(&self.version[..]).expect("Error writing version");
            buf.write_u16::<LittleEndian>(self.protocol).expect("Error writing protocol");
            buf.write_u8(category.into()).expect("Error writing payload category");
        }

        additional_data
    }
}

pub trait AwaitToken {
    /// Reads the connection token off the channel, parses the contents and returns the client id.
    fn read_connection_token(&mut self, secret_key: &[u8; 32]) -> Result<ClientId>;
}

impl AwaitToken for Channel {
    fn read_connection_token(&mut self, secret_key: &[u8; 32]) -> Result<ClientId> {
        let token = ConnectionToken::read(self.read_buffer.read_slice(), secret_key)?;

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

        self.read_buffer.move_head(ConnectionToken::SIZE);
        Ok(token.data.client_id)
    }
}

pub trait Connected {
    const HEADER_SIZE: usize = 11;

    fn read(&mut self) -> Result<Frame>;
    fn write<P: Serializable>(&mut self, payload: P, category: Category) -> Result<()>;
}

impl Connected for Channel {
    fn read(&mut self) -> Result<Frame> {
        let mut stream = self.read_buffer.read_slice();

        // Wait until there is enough data for the header
        if stream.len() < Self::HEADER_SIZE {
            return Err(Error::Wait);
        }

        // Read header
        let category = Category::from_byte(stream.read_u8()?)?;
        let sequence = stream.read_u64::<BigEndian>()?;
        let payload_size = stream.read_u16::<BigEndian>()? as usize;

        // Return early if the payload size is zero
        if payload_size == 0 {
            return Ok(Frame {
                category,
                data: &[],
            });
        }

        // Bail out if the payload cannot possibly fit in the buffer along with the header
        if payload_size > BUF_SIZE - Self::HEADER_SIZE {
            return Err(Error::PayloadTooLarge);
        }

        // Bail out if the sequence number is incorrect (duplicate or missing message)
        if sequence != self.client_sequence {
            return Err(Error::SequenceMismatch);
        }

        if stream.len() < payload_size {
            return Err(Error::Wait);
        }

        // Adjust for the MAC
        let decrypted_size = payload_size - crypto::MAC_SIZE;
        let additional_data = self.additional_data(category);

        // Read payload
        crypto::decrypt(
            &mut self.payload[..decrypted_size],
            &stream[..payload_size],
            &additional_data,
            sequence,
            &self.server_key,
        )?;

        self.read_buffer.move_head(Self::HEADER_SIZE + payload_size);
        self.client_sequence += 1;

        Ok(Frame {
            category,
            data: &self.payload[..decrypted_size],
        })
    }

    fn write<P: Serializable>(&mut self, payload: P, category: Category) -> Result<()> {
        let mut cursor = Cursor::new(&mut self.payload[..]);

        payload.serialize(&mut cursor)?;

        let payload_size = cursor.position() as usize;
        let encrypted_size = payload_size + crypto::MAC_SIZE;
        let total_size = Self::HEADER_SIZE + encrypted_size;

        if total_size > BUF_SIZE {
            panic!("Payload larger than the write buffer")
        }

        if total_size > self.write_buffer.free_capacity() {
            return Err(Error::Wait);
        }

        let additional_data = self.additional_data(category);
        let mut stream = self.write_buffer.write_slice();

        // Write header
        stream.write_u8(category.into())?;
        stream.write_u64::<BigEndian>(self.server_sequence)?;
        stream.write_u16::<BigEndian>(encrypted_size as u16)?;

        // Write payload
        crypto::encrypt(
            &mut stream[..encrypted_size],
            &self.payload[..payload_size],
            &additional_data,
            self.server_sequence,
            &self.client_key,
        )?;

        self.write_buffer.move_tail(total_size);
        self.server_sequence += 1;

        Ok(())
    }
}

/// Connection token sent by the client as part of the handshake process.
pub struct ConnectionToken {
    pub version: [u8; 16],
    pub protocol: u16,
    pub created: u64,
    pub expires: u64,
    pub sequence: u64,
    pub data: PrivateData,
}

impl ConnectionToken {
    pub const SIZE: usize = 43 + PrivateData::SIZE + crypto::MAC_SIZE;

    /// Read in the connection token form the supplied stream and decrypt the private
    /// data using the secret key.
    pub fn read(mut stream: &[u8], secret_key: &[u8; 32]) -> Result<ConnectionToken> {
        // Bail out immediately in case there isn't enough data in the buffer.
        if stream.len() < Self::SIZE {
            return Err(Error::Wait);
        }

        // Parse the data into the token structure.
        let mut instance = unsafe { mem::uninitialized::<ConnectionToken>() };

        stream.read_exact(&mut instance.version)?;
        instance.protocol = stream.read_u16::<BigEndian>()?;
        instance.created = stream.read_u64::<BigEndian>()?;
        instance.expires = stream.read_u64::<BigEndian>()?;
        instance.sequence = stream.read_u64::<BigEndian>()?;

        // Extract out the encrypted private data part.
        let mut plain = [0u8; PrivateData::SIZE];

        // Construct the additional data used for the encryption.
        let mut additional_data = [0u8; 26];
        {
            let mut additional_data_slice = &mut additional_data[..];

            additional_data_slice.write_all(&instance.version)?;
            additional_data_slice.write_u16::<LittleEndian>(instance.protocol)?;
            additional_data_slice.write_u64::<LittleEndian>(instance.created)?;
        }

        // Decrypt the cipher into the plain data.
        crypto::decrypt(
            &mut plain,
            &stream[..PrivateData::SIZE + crypto::MAC_SIZE],
            &additional_data,
            instance.sequence,
            &secret_key,
        )?;

        // Deserialize the private data part.
        instance.data = PrivateData::read(&plain[..])?;

        Ok(instance)
    }
}

/// Private data part (visible only to the server) of the connection token.
pub struct PrivateData {
    pub client_id: u64,
    pub server_key: [u8; 32],
    pub client_key: [u8; 32],
}

impl PrivateData {
    pub const SIZE: usize = 72;

    /// Parse the supplied stream as a private data structure.
    #[inline]
    fn read<R: Read>(mut stream: R) -> Result<PrivateData> {
        let mut instance = unsafe { mem::uninitialized::<PrivateData>() };

        instance.client_id = stream.read_u64::<BigEndian>()?;
        stream.read_exact(&mut instance.server_key)?;
        stream.read_exact(&mut instance.client_key)?;

        Ok(instance)
    }
}

/// Message category.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum Category {
    ConnectionAccepted = 0,
    ConnectionClosed = 1,
    Payload = 2,
}

impl Category {
    /// Convert a byte to a category.
    pub fn from_byte(byte: u8) -> Result<Category> {
        match byte {
            0 => Ok(Category::ConnectionAccepted),
            1 => Ok(Category::ConnectionClosed),
            2 => Ok(Category::Payload),
            _ => Err(Error::IncorrectCategory),
        }
    }
}

impl From<Category> for u8 {
    fn from(cls: Category) -> Self {
        cls as u8
    }
}

pub struct Frame<'a> {
    pub category: Category,
    pub data: &'a [u8],
}

#[cfg(test)]
mod tests {
    use super::*;
}