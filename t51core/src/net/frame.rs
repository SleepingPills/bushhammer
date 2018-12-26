use std::io;
use crate::net::buffer::Buffer;
use crate::net::crypto;

pub struct ConnectionToken {
    pub class: u8,
    pub version: [u8; 16],
    pub protocol: u64,
    pub created: u64,
    pub expires: u64,
    pub sequence: u64,
    pub mac: [u8; crypto::MAC_SIZE],
    pub data: PrivateData,
}

impl ConnectionToken {
    pub fn deserialize(buffer: &mut Buffer, secret_key: &[u8; 32]) -> io::Result<ConnectionToken> {
        unimplemented!()
    }
}

pub struct PrivateData {
    pub client_id: u64,
    pub server_key: [u8; 32],
    pub client_key: [u8; 32],
}

impl PrivateData {
    pub fn deserialize(buffer: &mut Buffer) -> io::Result<PrivateData> {
        unimplemented!()
    }
}

pub struct PacketHeader {
    pub class: u8,
    pub sequence: u64,
    pub size: u16,
    pub mac: [u8; crypto::MAC_SIZE],
}

impl PacketHeader {
    pub const SIZE: usize = 11;
}