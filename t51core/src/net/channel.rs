use crate::net::buffer::{Buffer, BUF_SIZE};
use crate::net::crypto;
use crate::net::result::{Error, Result};
use crate::net::shared::{current_timestamp, ClientId};
use byteorder::{BigEndian, LittleEndian, ReadBytesExt, WriteBytesExt};
use std::io::{Read, Write};
use std::mem;
use std::net::TcpStream;

pub struct Channel {
    // Tcp Stream
    stream: TcpStream,

    // Validation
    version: [u8; 16],
    protocol: u16,
    additional_data: [u8; 19],

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

        let mut additional_data = [0u8; 19];
        {
            let mut buf = &mut additional_data[..];
            buf.write_all(&version[..]).expect("Error writing version");
            buf.write_u16::<LittleEndian>(protocol).expect("Error writing protocol");
            buf.write_u8(PAYLOAD_CLASS).expect("Error writing payload class");
        }

        Channel {
            stream,
            version,
            protocol,
            client_sequence: 0,
            server_sequence: 0,
            additional_data,
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
}

pub trait AwaitToken {
    /// Reads the connection token off the channel, parses the contents and returns the client id.
    fn read_connection_token(&mut self, secret_key: &[u8; 32]) -> Result<ClientId>;
    /// Writes a connection acceptance message to the channel
    fn write_accept_connection(&mut self) -> Result<()>;
    /// Writes a connection rejection message to the channel
    fn write_reject_connection(&mut self) -> Result<()>;
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

    fn write_accept_connection(&mut self) -> Result<()> {
        Header::write_conn_accepted_header(self.write_buffer.write_slice(), self.server_sequence)
    }

    fn write_reject_connection(&mut self) -> Result<()> {
        Header::write_conn_closed_header(self.write_buffer.write_slice(), self.server_sequence)
    }
}

pub trait Connected {
    fn read_payload(&mut self) -> Result<&[u8]>;
}

impl Connected for Channel {
    // TODO: Extend this so that it reads any sort of message (with class > 0).
    // This means that it should also return the class along with the frame reference
    // Maybe return a tuple?
    fn read_payload(&mut self) -> Result<&[u8]> {
        let stream = self.read_buffer.read_slice();

        if stream.len() < Header::HEADER_SIZE {
            return Err(Error::MoreDataNeeded);
        }

        let frame = Header::read(stream)?;

        if frame.class != PAYLOAD_CLASS {
            return Err(Error::ClassMismatch);
        }

        let payload_size = frame.payload_size as usize;

        // Bail out if the payload cannot possibly fit in the buffer along with the header
        if payload_size > BUF_SIZE - Header::HEADER_SIZE {
            return Err(Error::PayloadTooLarge);
        }

        // Bail out if the sequence number is incorrect (duplicate or missing message)
        if frame.sequence != self.client_sequence {
            return Err(Error::SequenceMismatch);
        }

        if stream.len() < payload_size {
            return Err(Error::MoreDataNeeded);
        }

        let decrypted_size = payload_size - crypto::MAC_SIZE;

        crypto::decrypt(
            &mut self.payload[..decrypted_size],
            &stream[..payload_size],
            &self.additional_data,
            frame.sequence,
            &self.server_key,
        )?;

        self.read_buffer.move_head(Header::HEADER_SIZE + payload_size);
        self.client_sequence += 1;

        Ok(&self.payload[..decrypted_size])
    }
}

pub const CONN_TOKEN_CLASS: u8 = 0;
pub const CONN_ACCEPTED_CLASS: u8 = 1;
pub const CONN_CLOSED_CLASS: u8 = 2;
pub const PAYLOAD_CLASS: u8 = 3;

pub struct ConnectionToken {
    pub class: u8,
    pub version: [u8; 16],
    pub protocol: u16,
    pub created: u64,
    pub expires: u64,
    pub sequence: u64,
    pub data: PrivateData,
}

impl ConnectionToken {
    pub const SIZE: usize = 43 + PrivateData::SIZE + crypto::MAC_SIZE;

    pub fn read(mut stream: &[u8], secret_key: &[u8; 32]) -> Result<ConnectionToken> {
        // Bail out immediately in case there isn't enough data in the buffer.
        if stream.len() < Self::SIZE {
            return Err(Error::MoreDataNeeded);
        }

        // Parse the data into the token structure.
        let mut instance = unsafe { mem::uninitialized::<ConnectionToken>() };

        if stream.read_u8()? != CONN_TOKEN_CLASS {
            return Err(Error::ClassMismatch);
        }

        instance.class = CONN_TOKEN_CLASS;
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

pub struct PrivateData {
    pub client_id: u64,
    pub server_key: [u8; 32],
    pub client_key: [u8; 32],
}

impl PrivateData {
    pub const SIZE: usize = 72;

    #[inline]
    fn read<R: Read>(mut stream: R) -> Result<PrivateData> {
        let mut instance = unsafe { mem::uninitialized::<PrivateData>() };

        instance.client_id = stream.read_u64::<BigEndian>()?;
        stream.read_exact(&mut instance.server_key)?;
        stream.read_exact(&mut instance.client_key)?;

        Ok(instance)
    }
}

pub struct Header {
    pub class: u8,
    pub sequence: u64,
    pub payload_size: u16,
}

impl Header {
    pub const HEADER_SIZE: usize = 11;

    #[inline]
    pub fn read<R: Read>(mut stream: R) -> Result<Header> {
        Ok(Header {
            class: stream.read_u8()?,
            sequence: stream.read_u64::<BigEndian>()?,
            payload_size: stream.read_u16::<BigEndian>()?,
        })
    }

    #[inline]
    pub fn write_payload_header<W: Write>(mut stream: W, sequence: u64, payload_size: u16) -> Result<()> {
        stream.write_u8(PAYLOAD_CLASS)?;
        stream.write_u64::<BigEndian>(sequence)?;
        stream.write_u16::<BigEndian>(payload_size)?;
        Ok(())
    }

    #[inline]
    pub fn write_conn_accepted_header<W: Write>(mut stream: W, sequence: u64) -> Result<()> {
        stream.write_u8(CONN_ACCEPTED_CLASS)?;
        stream.write_u64::<BigEndian>(sequence)?;
        stream.write_u16::<BigEndian>(0)?;
        Ok(())
    }

    #[inline]
    pub fn write_conn_closed_header<W: Write>(mut stream: W, sequence: u64) -> Result<()> {
        stream.write_u8(CONN_CLOSED_CLASS)?;
        stream.write_u64::<BigEndian>(sequence)?;
        stream.write_u16::<BigEndian>(0)?;
        Ok(())
    }
}
