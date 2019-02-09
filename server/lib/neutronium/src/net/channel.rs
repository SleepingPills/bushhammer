use crate::net::buffer::Buffer;
use crate::net::frame::{Category, ControlFrame, Frame, PayloadInfo};
use crate::net::support::{Deserialize, ErrorType, NetworkError, NetworkResult, PayloadBatch, Serialize};
use byteorder::{BigEndian, LittleEndian, ReadBytesExt, WriteBytesExt};
use flux::crypto;
use flux::logging;
use flux::session::server::SessionKey;
use flux::session::user::PrivateData;
use flux::time::timestamp_secs;
use flux::UserId;
use mio::net::TcpStream;
use std::io;
use std::io::{Cursor, Read, Write};
use std::net::Shutdown;
use std::time::{Duration, Instant};

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

pub type ChannelId = usize;

#[derive(Debug, Copy, Clone, Eq, PartialEq)]
pub enum ChannelState {
    Handshake(Instant),
    Connected(UserId),
    Disconnected,
}

/// Represents a communication channel with a single endpoint. All communication on the channel
/// is encrypted.
pub struct Channel {
    id: Option<ChannelId>,

    // Tcp Stream
    stream: Option<TcpStream>,
    state: ChannelState,

    // Validation
    version: [u8; 16],
    protocol: u16,

    // Sequence of packets received from the client
    client_sequence: u64,
    // Sequence of packets sent to the client
    server_sequence: u64,

    // Communication Timestamps
    last_egress: Instant,
    last_ingress: Instant,

    // Client2Server Key
    server_key: [u8; crypto::KEY_SIZE],
    // Server2Client Key
    client_key: [u8; crypto::KEY_SIZE],

    // Channel Buffers
    read_buffer: Buffer,
    write_buffer: Buffer,

    // Payload buffer
    payload: [u8; PAYLOAD_BUF_SIZE],

    // Log
    log: logging::Logger,
}

impl Channel {
    /// Initializes a new channel with the supplied TcpStream, version and protocol.
    #[inline]
    pub fn new<'a, L: Into<Option<&'a logging::Logger>>>(
        version: [u8; 16],
        protocol: u16,
        log: L,
    ) -> Channel {
        let now = Instant::now();

        let channel_log = match log.into() {
            Some(log) => log.new(logging::o!()),
            _ => logging::Logger::root(logging::Discard, logging::o!()),
        };

        Channel {
            id: None,
            stream: None,
            state: ChannelState::Handshake(now),
            version,
            protocol,
            client_sequence: 0,
            server_sequence: 0,
            last_egress: now,
            last_ingress: now,
            server_key: Self::random_key(),
            client_key: Self::random_key(),
            read_buffer: Buffer::new(READ_BUF_SIZE),
            write_buffer: Buffer::new(WRITE_BUF_SIZE),
            payload: [0; PAYLOAD_BUF_SIZE],
            log: channel_log,
        }
    }

    /// Opens the channel using a new underlying stream. The channel must be closed for this
    /// operation to succeed.
    #[inline]
    pub fn open(&mut self, id: ChannelId, stream: TcpStream, now: Instant) {
        if self.state != ChannelState::Disconnected {
            panic!("Attempted to open an already open channel");
        }

        self.id = Some(id);
        self.state = ChannelState::Handshake(now);
        self.stream = Some(stream);

        logging::debug!(self.log, "channel opened"; "context" => "open", "channel_id" => self.id);
    }

    /// Closes the channel, the underlying stream and clears out all private data.
    #[inline]
    pub fn close(&mut self, notify: bool) {
        logging::debug!(self.log, "closing channel";
                        "context" => "close",
                        "channel_id" => self.id,
                        "client_sequence" => self.client_sequence,
                        "server_sequence" => self.server_sequence,
                        "last_egress" => ?self.last_egress,
                        "last_ingress" => ?self.last_ingress,
                        "read_size" => self.read_buffer.len(),
                        "write_size" => self.write_buffer.len());

        if notify {
            // Attempt to send a disconnection notice, but ignore any failures
            if let ChannelState::Connected(user_id) = self.state {
                logging::debug!(self.log, "notifying client"; "context" => "close", "channel_id" => self.id);
                drop(self.write_control(ControlFrame::ConnectionClosed(user_id)));
                drop(self.send_raw());
            }
        }

        // Only clear the buffers after the disconnect notification attempt was made. The data could be
        // corrupted otherwise.
        self.read_buffer.clear();
        self.write_buffer.clear();
        self.id = None;

        self.state = ChannelState::Disconnected;

        self.client_sequence = 0;
        self.server_sequence = 0;

        self.server_key = Self::random_key();
        self.client_key = Self::random_key();

        self.stream
            .take()
            .expect("Channel must have valid stream")
            .shutdown(Shutdown::Both)
            .unwrap_or_else(|err| panic!(err));

        logging::debug!(self.log, "channel closed"; "context" => "close", "channel_id" => self.id);
    }

    /// Returns the time elapsed since the last egress.
    #[inline]
    pub fn last_egress_elapsed(&self, now: Instant) -> Duration {
        now.duration_since(self.last_egress)
    }

    /// Returns the time elapsed since the last ingress.
    #[inline]
    pub fn last_ingress_elapsed(&self, now: Instant) -> Duration {
        now.duration_since(self.last_ingress)
    }

    /// Returns true if there is outgoing data on the channel.
    #[inline]
    pub fn has_egress(&self) -> bool {
        !self.write_buffer.is_empty()
    }

    /// Get the channel state.
    #[inline]
    pub fn get_state(&self) -> ChannelState {
        self.state
    }

    /// Registers this channel on the supplied poll.
    #[inline]
    pub fn register(&self, id: ChannelId, poll: &mio::Poll) -> NetworkResult<()> {
        logging::trace!(self.log, "registering channel on poll"; "context" => "register", "channel_id" => id);

        let result = poll.register(
            self.stream.as_ref().expect("Can't register disconnected channel"),
            id.into(),
            mio::Ready::readable() | mio::Ready::writable(),
            mio::PollOpt::edge(),
        )
        .map_err(Into::into);

        logging::debug!(self.log, "channel registered";
                        "context" => "register",
                        "channel_id" => id,
                        "result" => ?result);

        result
    }

    /// Deregisters this channel on the supplied poll.
    #[inline]
    pub fn deregister(&self, poll: &mio::Poll) -> NetworkResult<()> {
        logging::trace!(self.log, "deregistering channel on poll";
                        "context" => "deregister",
                        "channel_id" => self.id);

        let result = poll.deregister(
            self.stream
                .as_ref()
                .expect("Can't deregister disconnected channel"),
        )
        .map_err(Into::into);

        logging::debug!(self.log, "channel deregistered";
                        "context" => "deregister",
                        "channel_id" => self.id,
                        "result" => ?result);

        result
    }

    /// Read all available data off the network and updates the last ingress time if > 0 bytes have been
    /// transmitted.
    #[inline]
    pub fn receive(&mut self, now: Instant) -> NetworkResult<usize> {
        logging::trace!(self.log, "receiving data from network"; "context" => "receive", "channel_id" => self.id);

        let stream = &mut self.stream.as_ref().expect("Channel must have valid stream");

        let received = Self::fold_result(self.read_buffer.ingress(stream))?;

        if received > 0 {
            self.last_ingress = now;
        }

        Ok(received)
    }

    /// Send all the buffered data to the network and updates the last egress time if > 0 bytes have been
    /// transmitted.
    #[inline]
    pub fn send(&mut self, now: Instant) -> NetworkResult<usize> {
        logging::trace!(self.log, "sending data on the network"; "context" => "send", "channel_id" => self.id);

        if self.write_buffer.is_empty() {
            return Ok(0);
        }

        let sent = Self::fold_result(self.send_raw())?;

        if sent > 0 {
            self.last_egress = now;
        }

        Ok(sent)
    }

    /// Sends all the buffered data.
    #[inline]
    fn send_raw(&mut self) -> Result<usize, io::Error> {
        let stream = &mut self.stream.as_ref().expect("Channel must have valid stream");
        self.write_buffer.egress(stream)
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

    /// Generates a random key. Used for the initial setup.
    #[inline]
    fn random_key() -> [u8; crypto::KEY_SIZE] {
        let mut key = [0u8; crypto::KEY_SIZE];

        crypto::random_bytes(&mut key);

        key
    }

    /// Monomorphises the result to use the NetworkError plumbing and closes the channel in case
    /// a fatal error has occured.
    #[inline]
    fn fold_result<T, E: Into<NetworkError>>(result: Result<T, E>) -> NetworkResult<T> {
        match result {
            Ok(result) => Ok(result),
            Err(err) => Err(err.into()),
        }
    }
}

impl Channel {
    /// Write control data to the channel.
    pub fn write_control(&mut self, frame: ControlFrame) -> NetworkResult<()> {
        // Bail out if there isn't enough capacity to write the data
        if self.write_buffer.free_capacity() <= OVERHEAD_SIZE {
            return Err(NetworkError::Wait);
        }

        // Restrict payload size to account for header and mac
        let plain_payload_size = max_plain_payload_size(self.payload.len());

        let payload_slice = &mut self.payload[..plain_payload_size];

        let mut cursor = Cursor::new(payload_slice);

        let category = frame.category();
        frame.write(&mut cursor)?;
        let payload_size = cursor.position() as usize;

        self.write(payload_size, category)
    }

    /// Write payload data to the channel from a batch buffer.
    pub fn write_payload<P: Serialize>(&mut self, batch: &mut PayloadBatch<P>) -> NetworkResult<()> {
        // Bail out if there isn't enough capacity to write the data
        if self.write_buffer.free_capacity() <= OVERHEAD_SIZE {
            return Err(NetworkError::Wait);
        }

        // Restrict payload size to account for header and mac
        let plain_payload_size = max_plain_payload_size(self.write_buffer.free_capacity());

        let payload_slice = &mut self.payload[..plain_payload_size];

        let mut cursor = Cursor::new(payload_slice);
        batch.write(&mut cursor)?;
        let payload_size = cursor.position() as usize;

        self.write(payload_size, Category::Payload)
    }

    /// Write the current payload into the buffer
    fn write(&mut self, payload_size: usize, category: Category) -> NetworkResult<()> {
        let encrypted_size = payload_size + crypto::MAC_SIZE;
        let total_size = encrypted_size + HEADER_SIZE;

        logging::trace!(self.log, "writing message to output buffer";
                        "context" => "write",
                        "channel_id" => self.id,
                        "server_sequence" => self.server_sequence,
                        "write_buffer_capacity" => ?self.write_buffer.free_capacity(),
                        "plaintext_size" => ?payload_size,
                        "encrypted_size" => ?encrypted_size,
                        "total_size" => ?total_size);

        if total_size > self.write_buffer.free_capacity() {
            return Err(NetworkError::Wait);
        }

        let category_num = category as u8;

        let additional_data = self.additional_data(category_num);
        let mut stream = self.write_buffer.write_slice();

        // Write header
        stream.write_u8(category_num)?;
        stream.write_u64::<BigEndian>(self.server_sequence)?;
        stream.write_u16::<BigEndian>(encrypted_size as u16)?;

        logging::trace!(self.log, "encrypting message";
                        "context" => "write",
                        "channel_id" => self.id,
                        "server_sequence" => self.server_sequence);

        // Write payload
        if !crypto::encrypt(
            &mut stream[..encrypted_size],
            &self.payload[..payload_size],
            &additional_data,
            self.server_sequence,
            &self.client_key,
        ) {
            return Err(NetworkError::Fatal(ErrorType::Crypto));
        }

        self.write_buffer.move_tail(total_size);

        logging::trace!(self.log, "message written to output buffer";
                        "context" => "write",
                        "channel_id" => self.id,
                        "server_sequence" => self.server_sequence);

        self.server_sequence += 1;

        Ok(())
    }
}

impl Channel {
    /// Read the data on the channel into a frame. Only one frame will be returned at a time
    /// so this method should be called until NetworkResult::Wait is returned.
    ///
    /// Data for payload frames is retrieved by a follow up call to `read_payload`. The call must
    /// be made before calling `read` again, otherwise it will be overwritten by the next message.
    ///
    /// The channel will be automatically disconnected in case an error is encountered.
    #[inline]
    pub fn read(&mut self) -> NetworkResult<Frame> {
        let (size, category) = self.read_unpack()?;
        let result = Frame::read(&self.payload[..size], category);

        logging::trace!(self.log, "read in control frame";
                        "context" => "read",
                        "channel_id" => self.id,
                        "result" => ?result);

        result
    }

    /// Reads the payload into the supplied batch.
    ///
    /// The channel will be automatically disconnected in case an error is encountered.
    #[inline]
    pub fn read_payload<P: Deserialize>(
        &self,
        batch: &mut PayloadBatch<P>,
        pinfo: PayloadInfo,
    ) -> NetworkResult<()> {
        let mut cursor = Cursor::new(pinfo.select(&self.payload));

        logging::trace!(self.log, "reading payload frame";
                        "context" => "read_payload",
                        "channel_id" => self.id);

        let result = batch.read(&mut cursor);

        logging::trace!(self.log, "read in payload frame";
                        "context" => "read",
                        "channel_id" => self.id,
                        "result" => ?result);

        result
    }

    /// Read and unpack the data from the read buffer into the payload buffer.
    fn read_unpack(&mut self) -> Result<(usize, u8), NetworkError> {
        let mut stream = self.read_buffer.read_slice();

        logging::trace!(self.log, "reading message into the input buffer";
                        "context" => "read_unpack",
                        "channel_id" => self.id,
                        "client_sequence" => self.client_sequence);

        // Wait until there is enough data for the header
        if stream.len() < HEADER_SIZE {
            logging::trace!(self.log, "not enough data to parse the header";
                            "context" => "read_unpack",
                            "channel_id" => self.id,
                            "client_sequence" => self.client_sequence);

            return Err(NetworkError::Wait);
        }

        // Read header
        let category = stream.read_u8()?;
        let sequence = stream.read_u64::<BigEndian>()?;
        let payload_size = stream.read_u16::<BigEndian>()? as usize;

        logging::trace!(self.log, "read control message header";
                        "context" => "read_unpack",
                        "channel_id" => self.id,
                        "received_sequence" => sequence,
                        "client_sequence" => self.client_sequence,
                        "payload_size" => payload_size);

        // Bail out if the payload size is zero
        if payload_size == 0 {
            return Err(NetworkError::Fatal(ErrorType::EmptyPayload));
        }

        // Bail out if the payload cannot possibly fit in the buffer along with the header
        if payload_size > (READ_BUF_SIZE - HEADER_SIZE) {
            return Err(NetworkError::Fatal(ErrorType::PayloadTooLarge));
        }

        // Bail out if the sequence number is incorrect (duplicate or missing message)
        if sequence != self.client_sequence {
            return Err(NetworkError::Fatal(ErrorType::SequenceMismatch));
        }

        if stream.len() < payload_size {
            return Err(NetworkError::Wait);
        }

        // Adjust for the MAC
        let decrypted_size = payload_size - crypto::MAC_SIZE;
        let additional_data = self.additional_data(category);

        // Read payload
        if !crypto::decrypt(
            &mut self.payload[..decrypted_size],
            &stream[..payload_size],
            &additional_data,
            sequence,
            &self.server_key,
        ) {
            return Err(NetworkError::Fatal(ErrorType::Crypto));
        }

        self.read_buffer.move_head(HEADER_SIZE + payload_size);

        logging::trace!(self.log, "decrypted control message";
                        "context" => "read_unpack",
                        "channel_id" => self.id,
                        "received_sequence" => sequence,
                        "client_sequence" => self.client_sequence,
                        "decrypted_size" => decrypted_size);

        self.client_sequence += 1;

        Ok((decrypted_size, category))
    }
}

impl Channel {
    /// Reads the connection token off the channel, parses the contents and returns the client id.
    pub fn read_connection_token(&mut self, session_key: &SessionKey) -> Result<UserId, NetworkError> {
        let token = ConnectionToken::read(self.read_buffer.read_slice(), session_key)?;

        if token.expires < timestamp_secs() {
            return Err(NetworkError::Fatal(ErrorType::Expired));
        }

        if token.protocol != self.protocol {
            return Err(NetworkError::Fatal(ErrorType::ProtocolMismatch));
        }

        if token.version != self.version {
            return Err(NetworkError::Fatal(ErrorType::VersionMismatch));
        }

        self.server_key = token.data.server_key;
        self.client_key = token.data.client_key;

        self.read_buffer.move_head(ConnectionToken::SIZE);
        self.state = ChannelState::Connected(token.data.user_id);

        Ok(token.data.user_id)
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
    pub fn read(mut stream: &[u8], secret_key: &[u8; 32]) -> Result<ConnectionToken, NetworkError> {
        // Bail out immediately in case there isn't enough data in the buffer.
        if stream.len() < Self::SIZE {
            return Err(NetworkError::Wait);
        }

        // Parse the data into the token structure.
        let mut version: [u8; 16] = [0u8; 16];
        stream.read_exact(&mut version)?;
        let protocol = stream.read_u16::<BigEndian>()?;
        let expires = stream.read_u64::<BigEndian>()?;
        let sequence = stream.read_u64::<BigEndian>()?;

        // Extract out the encrypted private data part.
        let mut plain = [0u8; PrivateData::SIZE];

        // Construct the additional data used for the encryption.
        let additional_data = PrivateData::additional_data(&version, protocol, expires)?;

        // Decrypt the cipher into the plain data.
        if !crypto::decrypt(
            &mut plain,
            &stream[..PrivateData::SIZE + crypto::MAC_SIZE],
            &additional_data,
            sequence,
            &secret_key,
        ) {
            return Err(NetworkError::Fatal(ErrorType::Crypto));
        }

        let instance = ConnectionToken {
            version,
            protocol,
            expires,
            sequence,
            data: PrivateData::read(&plain[..])?,
        };

        Ok(instance)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::net::support::{Deserialize, SizedRead, SizedWrite};
    use std::mem;

    const VERSION: [u8; 16] = [5; 16];
    const PROTOCOL: u16 = 123;

    struct TestPayload(u64);

    impl Serialize for TestPayload {
        fn serialize<W: SizedWrite>(&self, stream: &mut W) -> Result<(), NetworkError> {
            match stream.free_capacity() >= 8 {
                true => stream.write_u64::<BigEndian>(self.0).map_err(Into::into),
                _ => Err(NetworkError::Wait),
            }
        }
    }

    impl Deserialize for TestPayload {
        fn deserialize<R: SizedRead>(stream: &mut R) -> Result<Self, NetworkError> {
            match stream.remaining_data() >= 8 {
                true => Ok(TestPayload(stream.read_u64::<BigEndian>()?)),
                _ => Err(NetworkError::Wait),
            }
        }
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

        let additional_data =
            PrivateData::additional_data(&token.version, token.protocol, token.expires).unwrap();

        crypto::encrypt(
            &mut stream[..PrivateData::SIZE + crypto::MAC_SIZE],
            &plain,
            &additional_data,
            token.sequence,
            key,
        );

        buffer.move_tail(ConnectionToken::SIZE);
    }

    #[test]
    fn test_additional_data() {
        let channel = Channel::new(VERSION, PROTOCOL, None);

        let ad = channel.additional_data(255);

        assert_eq!(&ad[..16], &[5u8; 16]);

        let mut reader = Cursor::new(&ad[16..]);

        assert_eq!(reader.read_u16::<LittleEndian>().unwrap(), 123);
        assert_eq!(reader.read_u8().unwrap(), 255);
    }

    #[test]
    fn test_read_connection_token() {
        let secret_key = SessionKey::new([33; crypto::KEY_SIZE]);

        let mut channel = Channel::new(VERSION, PROTOCOL, None);

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
        let secret_key = SessionKey::new([33; crypto::KEY_SIZE]);

        let mut channel = Channel::new(VERSION, PROTOCOL, None);

        channel
            .read_buffer
            .ingress(&[123u8; ConnectionToken::SIZE - 1][..])
            .unwrap();

        let result = channel.read_connection_token(&secret_key);

        assert_eq!(result.err().unwrap(), NetworkError::Wait);
        assert_eq!(channel.read_buffer.len(), ConnectionToken::SIZE - 1);
    }

    #[test]
    fn test_read_connection_token_err_expired() {
        let secret_key = SessionKey::new([33; crypto::KEY_SIZE]);

        let mut channel = Channel::new(VERSION, PROTOCOL, None);

        let mut token = make_connection_token();
        token.expires -= 7200;

        serialize_connection_token(&mut channel.read_buffer, &token, &secret_key);

        let result = channel.read_connection_token(&secret_key);

        assert_eq!(result.err().unwrap(), NetworkError::Fatal(ErrorType::Expired));
        assert_eq!(channel.read_buffer.len(), ConnectionToken::SIZE);
    }

    #[test]
    fn test_read_connection_token_err_version() {
        let secret_key = SessionKey::new([33; crypto::KEY_SIZE]);

        let mut channel = Channel::new(VERSION, PROTOCOL, None);

        let mut token = make_connection_token();
        token.version = [0u8; 16];

        serialize_connection_token(&mut channel.read_buffer, &token, &secret_key);

        let result = channel.read_connection_token(&secret_key);

        assert_eq!(
            result.err().unwrap(),
            NetworkError::Fatal(ErrorType::VersionMismatch)
        );
        assert_eq!(channel.read_buffer.len(), ConnectionToken::SIZE);
    }

    #[test]
    fn test_read_connection_token_err_protocol() {
        let secret_key = SessionKey::new([33; crypto::KEY_SIZE]);

        let mut channel = Channel::new(VERSION, PROTOCOL, None);

        let mut token = make_connection_token();
        token.protocol -= 1;

        serialize_connection_token(&mut channel.read_buffer, &token, &secret_key);

        let result = channel.read_connection_token(&secret_key);

        assert_eq!(
            result.unwrap_err(),
            NetworkError::Fatal(ErrorType::ProtocolMismatch)
        );
        assert_eq!(channel.read_buffer.len(), ConnectionToken::SIZE);
    }

    #[test]
    fn test_write_read_frame_roundtrip() {
        let mut channel = Channel::new(VERSION, PROTOCOL, None);

        channel.write_control(ControlFrame::Keepalive(123)).unwrap();

        assert_eq!(channel.server_sequence, 1);

        mem::swap(&mut channel.read_buffer, &mut channel.write_buffer);
        mem::swap(&mut channel.server_key, &mut channel.client_key);

        let response = channel.read().unwrap();

        match response {
            Frame::Control(ControlFrame::Keepalive(frame)) => assert_eq!(frame, 123),
            resp => panic!("Unexpected response {:?}", resp),
        };

        assert_eq!(channel.client_sequence, 1);
    }

    #[test]
    fn test_write_batch_read_batch_roundtrip() {
        let mut channel = Channel::new(VERSION, PROTOCOL, None);

        let expected_consumed_messages = 100;

        let mut outgoing = PayloadBatch::new();
        for i in 0..expected_consumed_messages {
            outgoing.push(TestPayload(i));
        }

        // Write out the batch
        channel.write_payload(&mut outgoing).unwrap();

        assert_eq!(outgoing.len(), 0);
        assert_eq!(channel.server_sequence, 1);

        mem::swap(&mut channel.read_buffer, &mut channel.write_buffer);
        mem::swap(&mut channel.server_key, &mut channel.client_key);

        let pinfo = match channel.read().unwrap() {
            Frame::Payload(pinfo) => pinfo,
            resp => panic!("Unexpected response {:?}", resp),
        };

        // Read out the messages into the receiving batch buffer
        let mut received = PayloadBatch::<TestPayload>::new();
        channel.read_payload(&mut received, pinfo).unwrap();

        assert_eq!(received.len(), expected_consumed_messages as usize);
        assert_eq!(channel.client_sequence, 1);
    }

    #[test]
    fn test_write_batch_partial() {
        let mut channel = Channel::new(VERSION, PROTOCOL, None);

        // The maximal number of messages that can fit in the write buffer
        let expected_consumed_messages = (WRITE_BUF_SIZE - OVERHEAD_SIZE) / 8;

        // Fill up the outgoing batch buffer with more messages than what can fit in the write buffer
        let mut outgoing = PayloadBatch::new();
        for i in 0..expected_consumed_messages * 2 {
            outgoing.push(TestPayload(i as u64));
        }

        // Write out the batch
        channel.write_payload(&mut outgoing).unwrap();

        assert_eq!(outgoing.len(), expected_consumed_messages);
        assert_eq!(channel.server_sequence, 1);
    }

    #[test]
    fn test_write_batch_zero() {
        let mut channel = Channel::new(VERSION, PROTOCOL, None);
        channel.write_buffer.move_tail(WRITE_BUF_SIZE - OVERHEAD_SIZE - 1);

        let mut outgoing = PayloadBatch::new();
        outgoing.push(TestPayload(1));

        // Write out the batch
        let result = channel.write_payload(&mut outgoing);

        assert_eq!(result.unwrap_err(), NetworkError::Wait);
        assert_eq!(outgoing.len(), 1);
        assert_eq!(channel.server_sequence, 0);
    }

    #[test]
    fn test_read_frame_zero_size() {
        let mut channel = Channel::new(VERSION, PROTOCOL, None);

        let mut stream = channel.read_buffer.write_slice();

        // Write header
        stream.write_u8(Category::Payload.into()).unwrap();
        stream.write_u64::<BigEndian>(0).unwrap();
        stream.write_u16::<BigEndian>(0).unwrap();

        channel.read_buffer.move_tail(HEADER_SIZE);

        let response = channel.read_unpack();

        assert_eq!(
            response.unwrap_err(),
            NetworkError::Fatal(ErrorType::EmptyPayload)
        );
    }

    #[test]
    fn test_read_frame_hdr_wait() {
        let mut channel = Channel::new(VERSION, PROTOCOL, None);

        let response = channel.read();

        assert_eq!(response.unwrap_err(), NetworkError::Wait);
    }

    #[test]
    fn test_read_frame_payload_wait() {
        let mut channel = Channel::new(VERSION, PROTOCOL, None);

        let mut stream = channel.read_buffer.write_slice();

        // Write header
        stream.write_u8(Category::Payload.into()).unwrap();
        stream.write_u64::<BigEndian>(0).unwrap();
        stream.write_u16::<BigEndian>(100).unwrap();

        // Write one byte less than expected size
        stream.write_all(&[0; 99]).unwrap();

        channel.read_buffer.move_tail(HEADER_SIZE + 99);

        let response = channel.read();

        assert_eq!(response.unwrap_err(), NetworkError::Wait);
    }

    #[test]
    fn test_read_frame_err_payload_size() {
        let mut channel = Channel::new(VERSION, PROTOCOL, None);

        let mut stream = channel.read_buffer.write_slice();

        // Write header
        stream.write_u8(Category::Payload.into()).unwrap();
        stream.write_u64::<BigEndian>(0).unwrap();
        stream.write_u16::<BigEndian>(u16::max_value()).unwrap();

        channel.read_buffer.move_tail(READ_BUF_SIZE);

        let response = channel.read_unpack();

        assert_eq!(
            response.unwrap_err(),
            NetworkError::Fatal(ErrorType::PayloadTooLarge)
        );
    }

    #[test]
    fn test_read_frame_err_sequence() {
        let mut channel = Channel::new(VERSION, PROTOCOL, None);

        let mut stream = channel.read_buffer.write_slice();

        // Write header
        stream.write_u8(Category::Payload.into()).unwrap();
        stream.write_u64::<BigEndian>(10).unwrap();
        stream.write_u16::<BigEndian>(5).unwrap();

        stream.write_all(&[0; 5]).unwrap();

        channel.read_buffer.move_tail(HEADER_SIZE + 5);

        let response = channel.read_unpack();

        assert_eq!(
            response.unwrap_err(),
            NetworkError::Fatal(ErrorType::SequenceMismatch)
        );
    }

    #[test]
    fn test_read_frame_err_crypto_key_mismatch() {
        let mut channel = Channel::new(VERSION, PROTOCOL, None);

        channel.write_control(ControlFrame::Keepalive(123)).unwrap();

        assert_eq!(channel.server_sequence, 1);

        // Swap the read/write buffers, but don't swap the keys
        mem::swap(&mut channel.read_buffer, &mut channel.write_buffer);

        let response = channel.read_unpack();

        assert_eq!(response.unwrap_err(), NetworkError::Fatal(ErrorType::Crypto));
    }

    #[test]
    fn test_read_frame_err_crypto_sequence_mismatch() {
        let mut channel = Channel::new(VERSION, PROTOCOL, None);

        channel.write_control(ControlFrame::Keepalive(123)).unwrap();

        let data = channel.write_buffer.data_slice();

        // Adjust the sequence
        data[8] += 1;
        channel.client_sequence = 1;

        // Swap both read/write buffers and client/server key so decryption proceeds correctly
        mem::swap(&mut channel.read_buffer, &mut channel.write_buffer);
        mem::swap(&mut channel.server_key, &mut channel.client_key);

        let response = channel.read_unpack();

        assert_eq!(response.unwrap_err(), NetworkError::Fatal(ErrorType::Crypto));
    }

    #[test]
    fn test_read_frame_err_crypto_version_mismatch() {
        let mut channel = Channel::new(VERSION, PROTOCOL, None);

        channel.write_control(ControlFrame::Keepalive(123)).unwrap();

        // Swap both read/write buffers and client/server key so decryption proceeds correctly
        mem::swap(&mut channel.read_buffer, &mut channel.write_buffer);
        mem::swap(&mut channel.server_key, &mut channel.client_key);

        // Muck about with the version
        channel.version[0] += 1;

        let response = channel.read_unpack();

        assert_eq!(response.unwrap_err(), NetworkError::Fatal(ErrorType::Crypto));
    }

    #[test]
    fn test_read_frame_err_crypto_protocol_mismatch() {
        let mut channel = Channel::new(VERSION, PROTOCOL, None);

        channel.write_control(ControlFrame::Keepalive(123)).unwrap();

        // Swap both read/write buffers and client/server key so decryption proceeds correctly
        mem::swap(&mut channel.read_buffer, &mut channel.write_buffer);
        mem::swap(&mut channel.server_key, &mut channel.client_key);

        // Muck about with the version
        channel.protocol += 1;

        let response = channel.read_unpack();

        assert_eq!(response.unwrap_err(), NetworkError::Fatal(ErrorType::Crypto));
    }

    #[test]
    fn test_read_frame_err_crypto_category_mismatch() {
        let mut channel = Channel::new(VERSION, PROTOCOL, None);

        channel.write_control(ControlFrame::Keepalive(123)).unwrap();

        let data = channel.write_buffer.data_slice();

        // Adjust the category
        data[0] += 1;

        // Swap both read/write buffers and client/server key so decryption proceeds correctly
        mem::swap(&mut channel.read_buffer, &mut channel.write_buffer);
        mem::swap(&mut channel.server_key, &mut channel.client_key);

        let response = channel.read_unpack();

        assert_eq!(response.unwrap_err(), NetworkError::Fatal(ErrorType::Crypto));
    }

    #[test]
    fn test_write_frame_wait() {
        let mut channel = Channel::new(VERSION, PROTOCOL, None);

        channel.write_buffer.move_tail(WRITE_BUF_SIZE - HEADER_SIZE);

        let result = channel.write_control(ControlFrame::Keepalive(123));

        assert_eq!(result.unwrap_err(), NetworkError::Wait);
    }
}
