use crate::net::buffer::{Buffer, BUF_SIZE};
use crate::net::crypto;
use crate::net::frame::Frame;
use crate::net::result::{Error, Result};
use crate::net::shared::{current_timestamp, ClientId, Serialize};
use byteorder::{BigEndian, LittleEndian, ReadBytesExt, WriteBytesExt};
use std::io::{Cursor, Read, Write};
use std::mem;
use std::net::TcpStream;

const HEADER_SIZE: usize = 11;
const MAX_CIPHER_PAYLOAD_SIZE: usize = BUF_SIZE - HEADER_SIZE;
const MAX_PLAIN_PAYLOAD_SIZE: usize = MAX_CIPHER_PAYLOAD_SIZE - crypto::MAC_SIZE;

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

    // Payload buffer must be shrunk by the header size and mac size to ensure that writers
    // do not put more data in it than the write buffer can hold.
    payload: [u8; MAX_PLAIN_PAYLOAD_SIZE],
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
            payload: [0; MAX_PLAIN_PAYLOAD_SIZE],
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
    fn additional_data(&self, category: u8) -> [u8; 19] {
        let mut additional_data = [0u8; 19];
        {
            let mut buf = &mut additional_data[..];
            buf.write_all(&self.version[..]).expect("Error writing version");
            buf.write_u16::<LittleEndian>(self.protocol)
                .expect("Error writing protocol");
            buf.write_u8(category).expect("Error writing payload category");
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
    fn read(&mut self) -> Result<Frame<&[u8]>>;
    fn write<P: Serialize>(&mut self, frame: Frame<P>) -> Result<()>;
}

impl Connected for Channel {
    fn read(&mut self) -> Result<Frame<&[u8]>> {
        let mut stream = self.read_buffer.read_slice();

        // Wait until there is enough data for the header
        if stream.len() < HEADER_SIZE {
            return Err(Error::Wait);
        }

        // Read header
        let category = stream.read_u8()?;
        let sequence = stream.read_u64::<BigEndian>()?;
        let payload_size = stream.read_u16::<BigEndian>()? as usize;

        // Bail out if the payload cannot possibly fit in the buffer along with the header
        if payload_size > MAX_CIPHER_PAYLOAD_SIZE {
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

        self.read_buffer.move_head(HEADER_SIZE + payload_size);
        self.client_sequence += 1;

        Frame::read(&self.payload[..decrypted_size], category)
    }

    fn write<P: Serialize>(&mut self, frame: Frame<P>) -> Result<()> {
        let mut cursor = Cursor::new(&mut self.payload[..]);

        let category = frame.category()?;
        frame.write(&mut cursor)?;

        let payload_size = cursor.position() as usize;
        let encrypted_size = payload_size + crypto::MAC_SIZE;
        let total_size = encrypted_size + HEADER_SIZE;

        if total_size > self.write_buffer.free_capacity() {
            return Err(Error::Wait);
        }

        let additional_data = self.additional_data(category);
        let mut stream = self.write_buffer.write_slice();

        // Write header
        stream.write_u8(category)?;
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
        instance.expires = stream.read_u64::<BigEndian>()?;
        instance.sequence = stream.read_u64::<BigEndian>()?;

        // Extract out the encrypted private data part.
        let mut plain = [0u8; PrivateData::SIZE];

        // Construct the additional data used for the encryption.
        let additional_data = instance.additional_data()?;

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

    fn additional_data(&self) -> Result<[u8; 26]> {
        let mut additional_data = [0u8; 26];
        let mut additional_data_slice = &mut additional_data[..];

        additional_data_slice.write_all(&self.version)?;
        additional_data_slice.write_u16::<LittleEndian>(self.protocol)?;
        additional_data_slice.write_u64::<LittleEndian>(self.expires)?;

        Ok(additional_data)
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

#[cfg(test)]
mod tests {
    use super::*;

    const VERSION: [u8; 16] = [5; 16];
    const PROTOCOL: u16 = 123;

    fn mock_stream() -> TcpStream {
        unsafe { mem::uninitialized::<TcpStream>() }
    }

    fn make_connection_token() -> ConnectionToken {
        ConnectionToken {
            version: VERSION,
            protocol: PROTOCOL,
            expires: current_timestamp() + 3600,
            sequence: 20,
            data: PrivateData {
                client_id: 8008,
                server_key: [15; crypto::KEY_SIZE],
                client_key: [101; crypto::KEY_SIZE],
            },
        }
    }

    fn serialize_connection_token(
        buffer: &mut Buffer,
        token: &ConnectionToken,
        key: &[u8; crypto::KEY_SIZE],
    ) {
        let mut stream = buffer.write_slice();

        stream.write_all(&token.version).unwrap();
        stream.write_u16::<BigEndian>(token.protocol).unwrap();
        stream.write_u64::<BigEndian>(token.expires).unwrap();
        stream.write_u64::<BigEndian>(token.sequence).unwrap();

        let mut plain = [0u8; PrivateData::SIZE];
        let mut private_data_stream = &mut plain[..];

        private_data_stream
            .write_u64::<BigEndian>(token.data.client_id)
            .unwrap();
        private_data_stream.write_all(&token.data.server_key).unwrap();
        private_data_stream.write_all(&token.data.client_key).unwrap();

        let additional_data = token.additional_data().unwrap();

        crypto::encrypt(
            &mut stream[..PrivateData::SIZE + crypto::MAC_SIZE],
            &plain,
            &additional_data,
            token.sequence,
            key,
        )
        .unwrap();

        buffer.move_tail(ConnectionToken::SIZE);
    }

    #[test]
    fn test_additional_data() {
        let channel = Channel::new(mock_stream(), VERSION, PROTOCOL);

        let ad = channel.additional_data(255);

        assert_eq!(&ad[..16], &[5u8; 16]);

        let mut reader = Cursor::new(&ad[16..]);

        assert_eq!(reader.read_u16::<LittleEndian>().unwrap(), 123);
        assert_eq!(reader.read_u8().unwrap(), 255);
    }

    #[test]
    fn test_read_connection_token() {
        let secret_key = [33; crypto::KEY_SIZE];

        let mut channel = Channel::new(mock_stream(), VERSION, PROTOCOL);

        let token = make_connection_token();

        serialize_connection_token(&mut channel.read_buffer, &token, &secret_key);

        let client_id = channel.read_connection_token(&secret_key).unwrap();

        assert_eq!(client_id, token.data.client_id);
        assert_eq!(channel.server_key, token.data.server_key);
        assert_eq!(channel.client_key, token.data.client_key);
        assert_eq!(channel.read_buffer.len(), 0);
    }

    #[test]
    fn test_read_connection_token_err_wait() {
        let secret_key = [33; crypto::KEY_SIZE];

        let mut channel = Channel::new(mock_stream(), VERSION, PROTOCOL);

        channel.read_buffer.ingress(&[123u8; ConnectionToken::SIZE - 1][..]).unwrap();

        let result = channel.read_connection_token(&secret_key);

        assert_eq!(result.err().unwrap(), Error::Wait);
        assert_eq!(channel.read_buffer.len(), ConnectionToken::SIZE - 1);
    }

    #[test]
    fn test_read_connection_token_err_expired() {
        let secret_key = [33; crypto::KEY_SIZE];

        let mut channel = Channel::new(mock_stream(), VERSION, PROTOCOL);

        let mut token = make_connection_token();
        token.expires -= 7200;

        serialize_connection_token(&mut channel.read_buffer, &token, &secret_key);

        let result = channel.read_connection_token(&secret_key);

        assert_eq!(result.err().unwrap(), Error::Expired);
        assert_eq!(channel.read_buffer.len(), ConnectionToken::SIZE);
    }

    #[test]
    fn test_read_connection_token_err_version() {
        let secret_key = [33; crypto::KEY_SIZE];

        let mut channel = Channel::new(mock_stream(), VERSION, PROTOCOL);

        let mut token = make_connection_token();
        token.version = [0u8; 16];

        serialize_connection_token(&mut channel.read_buffer, &token, &secret_key);

        let result = channel.read_connection_token(&secret_key);

        assert_eq!(result.err().unwrap(), Error::VersionMismatch);
        assert_eq!(channel.read_buffer.len(), ConnectionToken::SIZE);
    }

    #[test]
    fn test_read_connection_token_err_protocol() {
        let secret_key = [33; crypto::KEY_SIZE];

        let mut channel = Channel::new(mock_stream(), VERSION, PROTOCOL);

        let mut token = make_connection_token();
        token.protocol -= 1;

        serialize_connection_token(&mut channel.read_buffer, &token, &secret_key);

        let result = channel.read_connection_token(&secret_key);

        assert_eq!(result.err().unwrap(), Error::ProtocolMismatch);
        assert_eq!(channel.read_buffer.len(), ConnectionToken::SIZE);
    }

    #[test]
    fn test_write_read_frame_roundtrip() {
        let mut channel = Channel::new(mock_stream(), VERSION, PROTOCOL);
        channel.write()
        // Write: Ensure server sequence was bumped and write buffer tail was moved
        // Read: Ensure client sequence was bumped and read buffer head was moved
    }

    #[test]
    fn test_read_frame_err_hdr_wait() {}

    #[test]
    fn test_read_frame_err_payload_wait() {}

    #[test]
    fn test_read_frame_err_payload_size() {}

    #[test]
    fn test_read_frame_err_sequence() {}

    #[test]
    fn test_read_frame_err_crypto() {}

    #[test]
    fn test_write_frame_err_wait() {}
}
