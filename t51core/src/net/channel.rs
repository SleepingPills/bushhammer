use crate::net::buffer::Buffer;
use crate::net::crypto;
use crate::net::frame::{Category, Frame};
use crate::net::result::{Error, Result};
use crate::net::shared::{PayloadBatch, Serialize, UserId};
use byteorder::{BigEndian, LittleEndian, ReadBytesExt, WriteBytesExt};
use std::io;
use std::io::{Cursor, Read, Write};
use std::mem;
use std::net::{Shutdown, TcpStream};
use std::time::SystemTime;

// Write buffer should be 512k
const WRITE_BUF_SIZE: usize = 8 * 65536;
const READ_BUF_SIZE: usize = 65536;
// Use the write buffer as it is bigger
const PAYLOAD_BUF_SIZE: usize = WRITE_BUF_SIZE;

const HEADER_SIZE: usize = 11;
const OVERHEAD_SIZE: usize = HEADER_SIZE + crypto::MAC_SIZE;

const fn max_plain_payload_size(capacity: usize) -> usize {
    capacity - OVERHEAD_SIZE
}

/// Represents a communication channel with a single endpoint. All communication on the channel
/// is encrypted.
pub struct Channel {
    // Tcp Stream
    stream: TcpStream,
    open: bool,

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

    // Payload buffer
    payload: [u8; PAYLOAD_BUF_SIZE],
}

impl Channel {
    /// Initializes a new channel with the supplied TcpStream, version and protocol.
    #[inline]
    pub fn new(stream: TcpStream, version: [u8; 16], protocol: u16) -> Channel {
        Channel {
            stream,
            open: true,
            version,
            protocol,
            client_sequence: 0,
            server_sequence: 0,
            server_key: Self::random_key(),
            client_key: Self::random_key(),
            read_buffer: Buffer::new(READ_BUF_SIZE),
            write_buffer: Buffer::new(WRITE_BUF_SIZE),
            payload: [0; PAYLOAD_BUF_SIZE],
        }
    }

    /// Returns a boolean indicating whether there is data to be read in immediately.
    #[inline]
    pub fn pull_ready(&self) -> bool {
        self.read_buffer.len() > 0
    }

    /// Returns a boolean indicating whether there is data to be sent immediately.
    #[inline]
    pub fn push_ready(&self) -> bool {
        self.read_buffer.len() > 0
    }

    /// Closes the channel, the underlying stream and clears out all private data.
    #[inline]
    pub fn close(&mut self) -> Result<()> {
        self.open = false;

        self.client_sequence = 0;
        self.server_sequence = 0;

        self.clear();

        self.server_key = Self::random_key();
        self.client_key = Self::random_key();

        match self.stream.shutdown(Shutdown::Both) {
            Ok(_) => Ok(()),
            Err(ref error) if error.kind() == io::ErrorKind::NotConnected => Ok(()),
            Err(ref error) => Err(Error::Io(error.kind())),
        }
    }

    /// Opens the channel using a new underlying stream. The channel must be closed for this
    /// operation to succeed.
    #[inline]
    pub fn open(&mut self, stream: TcpStream) -> Result<()> {
        if self.open {
            return Err(Error::AlreadyConnected);
        }

        self.open = true;
        self.stream = stream;

        Ok(())
    }

    /// Clear the channel buffers
    #[inline]
    pub fn clear(&mut self) {
        self.read_buffer.clear();
        self.write_buffer.clear();
    }

    /// Send all the buffered data to the network.
    #[inline]
    pub fn send(&mut self) -> Result<usize> {
        self.write_buffer.egress(&mut self.stream).map_err(Into::into)
    }

    /// Read all available data off the network.
    #[inline]
    pub fn recieve(&mut self) -> Result<usize> {
        self.read_buffer.ingress(&mut self.stream).map_err(Into::into)
    }

    /// Write the current payload into the buffer
    fn write_payload(&mut self, payload_size: usize, category: u8) -> Result<()> {
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

    /// Constructs the array holding additional data
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

    #[inline]
    fn random_key() -> [u8; crypto::KEY_SIZE] {
        let mut key = [0u8; crypto::KEY_SIZE];

        crypto::random_bytes(&mut key);

        key
    }
}

/// Trait describing channels while in the state of waiting for the connection token.
pub trait Handshake {
    /// Reads the connection token off the channel, parses the contents and returns the client id.
    fn read_connection_token(&mut self, secret_key: &[u8; 32]) -> Result<UserId>;
}

impl Handshake for Channel {
    fn read_connection_token(&mut self, secret_key: &[u8; 32]) -> Result<UserId> {
        let token = ConnectionToken::read(self.read_buffer.read_slice(), secret_key)?;

        if token.expires < timestamp_secs() {
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
        Ok(token.data.user_id)
    }
}

/// Trait describing channels while in the fully connected state.
pub trait Connected {
    /// Read the data on the channel into a frame. Only one frame will be returned at a time
    /// so this method should be called until Error::Wait is returned.
    fn read(&mut self) -> Result<Frame<&[u8]>>;

    /// Write data to the channel.
    fn write<P: Serialize>(&mut self, frame: Frame<P>) -> Result<()>;

    /// Write data to the channel from a batch buffer
    fn write_batch<P: Serialize>(&mut self, batch_buffer: &mut PayloadBatch<P>) -> Result<()>;
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

        // Return immediately if the payload size is zero
        if payload_size == 0 {
            return Frame::read(&[], category);
        }

        // Bail out if the payload cannot possibly fit in the buffer along with the header
        if payload_size > (READ_BUF_SIZE - HEADER_SIZE) {
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
        // Bail out if there isn't enough capacity to write the data
        if self.write_buffer.free_capacity() <= OVERHEAD_SIZE {
            return Err(Error::Wait);
        }

        // Restrict payload size to account for header and mac
        let plain_payload_size = max_plain_payload_size(self.payload.len());

        let payload_slice = &mut self.payload[..plain_payload_size];

        let mut cursor = Cursor::new(payload_slice);

        let category = frame.category();
        frame.write(&mut cursor)?;
        let payload_size = cursor.position() as usize;

        self.write_payload(payload_size, category)
    }

    fn write_batch<P: Serialize>(&mut self, batch: &mut PayloadBatch<P>) -> Result<()> {
        // Bail out if there isn't enough capacity to write the data
        if self.write_buffer.free_capacity() <= OVERHEAD_SIZE {
            return Err(Error::Wait);
        }

        // Restrict payload size to account for header and mac
        let plain_payload_size = max_plain_payload_size(self.write_buffer.free_capacity());

        let payload_slice = &mut self.payload[..plain_payload_size];

        let mut cursor = Cursor::new(payload_slice);
        batch.write(&mut cursor)?;
        let payload_size = cursor.position() as usize;

        self.write_payload(payload_size, Category::Payload as u8)
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
    pub user_id: u64,
    pub server_key: [u8; 32],
    pub client_key: [u8; 32],
}

impl PrivateData {
    pub const SIZE: usize = 72;

    /// Parse the supplied stream as a private data structure.
    #[inline]
    fn read<R: Read>(mut stream: R) -> Result<PrivateData> {
        let mut instance = unsafe { mem::uninitialized::<PrivateData>() };

        instance.user_id = stream.read_u64::<BigEndian>()?;
        stream.read_exact(&mut instance.server_key)?;
        stream.read_exact(&mut instance.client_key)?;

        Ok(instance)
    }
}

/// Returns the current unix timestamp (seconds elapsed since 1970-01-01)
#[inline]
pub fn timestamp_secs() -> u64 {
    SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .expect("Closed timelike curve, reality compromised")
        .as_secs()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::net::shared::{Deserialize, SizedRead, SizedWrite};

    const VERSION: [u8; 16] = [5; 16];
    const PROTOCOL: u16 = 123;

    struct TestPayload(u64);

    impl Serialize for TestPayload {
        fn serialize<W: SizedWrite>(&self, stream: &mut W) -> Result<()> {
            match stream.free_capacity() >= 8 {
                true => stream.write_u64::<BigEndian>(self.0).map_err(Into::into),
                _ => Err(Error::Wait),
            }
        }
    }

    impl Deserialize for TestPayload {
        fn deserialize<R: SizedRead>(stream: &mut R) -> Result<Self> {
            match stream.remaining_data() >= 8 {
                true => Ok(TestPayload(stream.read_u64::<BigEndian>()?)),
                _ => Err(Error::Wait),
            }
        }
    }

    fn mock_stream() -> TcpStream {
        unsafe { mem::uninitialized::<TcpStream>() }
    }

    fn make_connection_token() -> ConnectionToken {
        ConnectionToken {
            version: VERSION,
            protocol: PROTOCOL,
            expires: timestamp_secs() + 3600,
            sequence: 20,
            data: PrivateData {
                user_id: 8008,
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
            .write_u64::<BigEndian>(token.data.user_id)
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

        let user_id = channel.read_connection_token(&secret_key).unwrap();

        assert_eq!(user_id, token.data.user_id);
        assert_eq!(channel.server_key, token.data.server_key);
        assert_eq!(channel.client_key, token.data.client_key);
        assert_eq!(channel.read_buffer.len(), 0);
    }

    #[test]
    fn test_read_connection_token_err_wait() {
        let secret_key = [33; crypto::KEY_SIZE];

        let mut channel = Channel::new(mock_stream(), VERSION, PROTOCOL);

        channel
            .read_buffer
            .ingress(&[123u8; ConnectionToken::SIZE - 1][..])
            .unwrap();

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

        let payload = 123123123;

        channel.write(Frame::Payload(TestPayload(payload))).unwrap();

        assert_eq!(channel.server_sequence, 1);

        mem::swap(&mut channel.read_buffer, &mut channel.write_buffer);
        mem::swap(&mut channel.server_key, &mut channel.client_key);

        let response = channel.read().unwrap();

        let mut stream = match response {
            Frame::Payload(stream) => stream,
            _ => panic!("Unexpected frame type"),
        };

        let received_payload = stream.read_u64::<BigEndian>().unwrap();

        assert_eq!(received_payload, payload);
        assert_eq!(channel.client_sequence, 1);
    }

    #[test]
    fn test_write_read_batch_roundtrip() {
        let mut channel = Channel::new(mock_stream(), VERSION, PROTOCOL);

        let expected_consumed_messages = 100;

        let mut outgoing = PayloadBatch::new();
        for i in 0..expected_consumed_messages {
            outgoing.push(TestPayload(i));
        }

        // Write out the batch
        channel.write_batch(&mut outgoing).unwrap();

        assert_eq!(outgoing.len(), 0);
        assert_eq!(channel.server_sequence, 1);

        mem::swap(&mut channel.read_buffer, &mut channel.write_buffer);
        mem::swap(&mut channel.server_key, &mut channel.client_key);

        let response = channel.read().unwrap();

        let stream = match response {
            Frame::Payload(stream) => stream,
            _ => panic!("Unexpected frame type"),
        };

        // Read out the messages into the receiving batch buffer
        let mut received = PayloadBatch::<TestPayload>::new();
        received.read(&mut Cursor::new(stream)).unwrap();

        assert_eq!(received.len(), expected_consumed_messages as usize);
        assert_eq!(channel.client_sequence, 1);
    }

    #[test]
    fn test_write_batch_partial() {
        let mut channel = Channel::new(mock_stream(), VERSION, PROTOCOL);

        // The maximal number of messages that can fit in the write buffer
        let expected_consumed_messages = (WRITE_BUF_SIZE - OVERHEAD_SIZE) / 8;

        // Fill up the outgoing batch buffer with more messages than what can fit in the write buffer
        let mut outgoing = PayloadBatch::new();
        for i in 0..expected_consumed_messages * 2 {
            outgoing.push(TestPayload(i as u64));
        }

        // Write out the batch
        channel.write_batch(&mut outgoing).unwrap();

        assert_eq!(outgoing.len(), expected_consumed_messages);
        assert_eq!(channel.server_sequence, 1);
    }

    #[test]
    fn test_write_batch_zero() {
        let mut channel = Channel::new(mock_stream(), VERSION, PROTOCOL);
        channel.write_buffer.move_tail(WRITE_BUF_SIZE - OVERHEAD_SIZE - 1);

        // Fill up the outgoing batch buffer with more messages than what can fit in the write buffer
        let mut outgoing = PayloadBatch::new();
        outgoing.push(TestPayload(1));

        // Write out the batch
        let result = channel.write_batch(&mut outgoing);

        assert_eq!(result.err().unwrap(), Error::Wait);
        assert_eq!(outgoing.len(), 1);
        assert_eq!(channel.server_sequence, 0);
    }

    #[test]
    fn test_read_frame_zero_size() {
        let mut channel = Channel::new(mock_stream(), VERSION, PROTOCOL);

        let mut stream = channel.read_buffer.write_slice();

        // Write header
        stream.write_u8(2).unwrap();
        stream.write_u64::<BigEndian>(0).unwrap();
        stream.write_u16::<BigEndian>(0).unwrap();

        channel.read_buffer.move_tail(HEADER_SIZE);

        let stream = match channel.read().unwrap() {
            Frame::Payload(stream) => stream,
            _ => panic!("Unexpected frame type"),
        };

        assert_eq!(stream, &[0u8; 0]);
    }

    #[test]
    fn test_read_frame_err_hdr_wait() {
        let mut channel = Channel::new(mock_stream(), VERSION, PROTOCOL);

        let response = channel.read();

        assert_eq!(response.err().unwrap(), Error::Wait);
    }

    #[test]
    fn test_read_frame_err_payload_wait() {
        let mut channel = Channel::new(mock_stream(), VERSION, PROTOCOL);

        let mut stream = channel.read_buffer.write_slice();

        // Write header
        stream.write_u8(2).unwrap();
        stream.write_u64::<BigEndian>(0).unwrap();
        stream.write_u16::<BigEndian>(100).unwrap();

        // Write one byte less than expected size
        stream.write_all(&[0; 99]).unwrap();

        channel.read_buffer.move_tail(HEADER_SIZE + 99);

        let response = channel.read();

        assert_eq!(response.err().unwrap(), Error::Wait);
    }

    #[test]
    fn test_read_frame_err_payload_size() {
        let mut channel = Channel::new(mock_stream(), VERSION, PROTOCOL);

        let mut stream = channel.read_buffer.write_slice();

        // Write header
        stream.write_u8(2).unwrap();
        stream.write_u64::<BigEndian>(0).unwrap();
        stream.write_u16::<BigEndian>(u16::max_value()).unwrap();

        channel.read_buffer.move_tail(READ_BUF_SIZE);

        let response = channel.read();

        assert_eq!(response.err().unwrap(), Error::PayloadTooLarge);
    }

    #[test]
    fn test_read_frame_err_sequence() {
        let mut channel = Channel::new(mock_stream(), VERSION, PROTOCOL);

        let mut stream = channel.read_buffer.write_slice();

        // Write header
        stream.write_u8(2).unwrap();
        stream.write_u64::<BigEndian>(10).unwrap();
        stream.write_u16::<BigEndian>(5).unwrap();

        stream.write_all(&[0; 5]).unwrap();

        channel.read_buffer.move_tail(HEADER_SIZE + 5);

        let response = channel.read();

        assert_eq!(response.err().unwrap(), Error::SequenceMismatch);
    }

    #[test]
    fn test_read_frame_err_crypto_key_mismatch() {
        let mut channel = Channel::new(mock_stream(), VERSION, PROTOCOL);

        let payload = 123123123;

        channel.write(Frame::Payload(TestPayload(payload))).unwrap();

        assert_eq!(channel.server_sequence, 1);

        // Swap the read/write buffers, but don't swap the keys
        mem::swap(&mut channel.read_buffer, &mut channel.write_buffer);

        let response = channel.read();

        assert_eq!(response.err().unwrap(), Error::Crypto);
    }

    #[test]
    fn test_read_frame_err_crypto_sequence_mismatch() {
        let mut channel = Channel::new(mock_stream(), VERSION, PROTOCOL);

        let payload = 123123123;

        channel.write(Frame::Payload(TestPayload(payload))).unwrap();

        let data = channel.write_buffer.data_slice();

        // Adjust the sequence
        data[8] += 1;
        channel.client_sequence = 1;

        // Swap both read/write buffers and client/server key so decryption proceeds correctly
        mem::swap(&mut channel.read_buffer, &mut channel.write_buffer);
        mem::swap(&mut channel.server_key, &mut channel.client_key);

        let response = channel.read();

        assert_eq!(response.err().unwrap(), Error::Crypto);
    }

    #[test]
    fn test_read_frame_err_crypto_version_mismatch() {
        let mut channel = Channel::new(mock_stream(), VERSION, PROTOCOL);

        let payload = 123123123;

        channel.write(Frame::Payload(TestPayload(payload))).unwrap();

        // Swap both read/write buffers and client/server key so decryption proceeds correctly
        mem::swap(&mut channel.read_buffer, &mut channel.write_buffer);
        mem::swap(&mut channel.server_key, &mut channel.client_key);

        // Muck about with the version
        channel.version[0] += 1;

        let response = channel.read();

        assert_eq!(response.err().unwrap(), Error::Crypto);
    }

    #[test]
    fn test_read_frame_err_crypto_protocol_mismatch() {
        let mut channel = Channel::new(mock_stream(), VERSION, PROTOCOL);

        let payload = 123123123;

        channel.write(Frame::Payload(TestPayload(payload))).unwrap();

        // Swap both read/write buffers and client/server key so decryption proceeds correctly
        mem::swap(&mut channel.read_buffer, &mut channel.write_buffer);
        mem::swap(&mut channel.server_key, &mut channel.client_key);

        // Muck about with the version
        channel.protocol += 1;

        let response = channel.read();

        assert_eq!(response.err().unwrap(), Error::Crypto);
    }

    #[test]
    fn test_read_frame_err_crypto_category_mismatch() {
        let mut channel = Channel::new(mock_stream(), VERSION, PROTOCOL);

        let payload = 123123123;

        channel.write(Frame::Payload(TestPayload(payload))).unwrap();

        let data = channel.write_buffer.data_slice();

        // Adjust the category
        data[0] += 1;

        // Swap both read/write buffers and client/server key so decryption proceeds correctly
        mem::swap(&mut channel.read_buffer, &mut channel.write_buffer);
        mem::swap(&mut channel.server_key, &mut channel.client_key);

        let response = channel.read();

        assert_eq!(response.err().unwrap(), Error::Crypto);
    }

    #[test]
    fn test_write_frame_err_wait() {
        let mut channel = Channel::new(mock_stream(), VERSION, PROTOCOL);

        const CIPHER_SIZE: usize = WRITE_BUF_SIZE - HEADER_SIZE;

        channel
            .write_buffer
            .write_slice()
            .write_all(&[0; CIPHER_SIZE])
            .unwrap();
        channel.write_buffer.move_tail(CIPHER_SIZE);

        let payload = 123123123;
        let result = channel.write(Frame::Payload(TestPayload(payload)));

        assert_eq!(result.err().unwrap(), Error::Wait);
    }
}
