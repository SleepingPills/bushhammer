use crate::net::buffer::Buffer;
use crate::net::error::{Error, TxResult};
use byteorder::{BigEndian, ReadBytesExt, WriteBytesExt};
use std::mem;

pub struct ConnectionToken {
    pub version: [u8; 16],
    pub protocol: u64,
    pub created: u64,
    pub expires: u64,
    pub sequence: u64,
    pub private: [u8; 72],
}

impl ConnectionToken {
    const SIZE: usize = 120;

    pub fn deserialize(buffer: &mut Buffer) -> TxResult<ConnectionToken> {
        match buffer.size_check(Self::SIZE) {
            true => {
                let mut instance = unsafe { mem::uninitialized::<ConnectionToken>() };
                buffer.egress(&mut instance.version[..])?;
                instance.protocol = buffer.read_u64::<BigEndian>()?;
                instance.created = buffer.read_u64::<BigEndian>()?;
                instance.expires = buffer.read_u64::<BigEndian>()?;
                instance.sequence = buffer.read_u64::<BigEndian>()?;
                buffer.egress(&mut instance.private[..])?;
                Ok(instance)
            }
            _ => Err(Error::NeedMore),
        }
    }
}

pub struct PrivateData {
    pub client_id: u64,
    pub server_key: [u8; 32],
    pub client_key: [u8; 32],
}

impl PrivateData {
    const SIZE: usize = 72;

    pub fn deserialize(mut buffer: &[u8]) -> TxResult<PrivateData> {
        match buffer.len() == Self::SIZE {
            true => {
                let mut instance = unsafe { mem::uninitialized::<PrivateData>() };
                instance.client_id = buffer.read_u64::<BigEndian>()?;
                let key_len = instance.server_key.len();
                instance.server_key.copy_from_slice(&buffer[..key_len]);
                instance.client_key.copy_from_slice(&buffer[key_len..(key_len * 2)]);
                Ok(instance)
            }
            _ => Err(Error::CorruptData),
        }
    }
}

pub struct ChallengeHeader {
    pub sequence: u64,
}

impl ChallengeHeader {
    const SIZE: usize = 8;
}

pub struct PayloadHeader {
    pub class: u8,
    pub sequence: u64,
    pub size: u16,
}

impl PayloadHeader {
    const SIZE: usize = 11;

    pub fn deserialize(buffer: &mut Buffer) -> TxResult<PayloadHeader> {
        match buffer.size_check(Self::SIZE) {
            true => {
                let mut instance = unsafe { mem::uninitialized::<PayloadHeader>() };
                instance.class = buffer.read_u8()?;
                instance.sequence = buffer.read_u64::<BigEndian>()?;
                instance.size = buffer.read_u16::<BigEndian>()?;
                Ok(instance)
            }
            _ => Err(Error::NeedMore),
        }
    }
}

pub struct Frame {
    pub header: PayloadHeader,
    pub data: Vec<u8>,
}
