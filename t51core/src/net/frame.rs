use std::io;
use crate::net::buffer::Buffer;

pub struct ConnectionToken {
    pub version: [u8; 16],
    pub protocol: u64,
    pub created: u64,
    pub expires: u64,
    pub sequence: u64,
    pub nonce: [u8; 24],
    pub data: PrivateData,
}

impl ConnectionToken {
    pub fn deserialize<R: io::Read>(buffer: &mut R) -> io::Result<ConnectionToken> {
        unimplemented!()
    }
}

pub struct PrivateData {
    pub client_id: u64,
    pub server_key: [u8; 32],
    pub client_key: [u8; 32],
}

impl PrivateData {
    pub fn deserialize<R: io::Read>(buffer: &mut R) -> io::Result<PrivateData> {
        unimplemented!()
    }
}

pub struct Header {
    pub class: u8,
    pub sequence: u64,
    pub size: u16,
}

impl Header {
    pub const SIZE: usize = 11;
}

pub struct Frame {
    pub header: Header,
    pub data: Vec<u8>,
}

impl Frame {
    pub fn serialize(&self, buffer: &mut Buffer) -> io::Result<()> {
        // Check that the buffer is big enough to send the frame, reject sending otherwise
        // We can do this because we know the data size and the header size is constant.
        // Write out the header
        // Encrypt the data into the buffer
        unimplemented!()
    }
}