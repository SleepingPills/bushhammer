use crate::net::shared::{ErrorType, NetworkError, NetworkResult, Serialize, SizedWrite, UserId};
use byteorder::{BigEndian, ReadBytesExt, WriteBytesExt};
use std::io;

pub enum Category {
    ConnectionAccepted = 0,
    ConnectionClosed = 1,
    Payload = 2,
    Keepalive = 3,
}

#[derive(Debug, Eq, PartialEq)]
pub enum Frame<P> {
    ConnectionAccepted(UserId),
    ConnectionClosed(UserId),
    Payload(P),
    Keepalive(UserId),
}

impl<P> Frame<P> {
    pub fn category(&self) -> u8 {
        match self {
            Frame::ConnectionAccepted(_) => Category::ConnectionAccepted as u8,
            Frame::ConnectionClosed(_) => Category::ConnectionClosed as u8,
            Frame::Payload(_) => Category::Payload as u8,
            Frame::Keepalive(_) => Category::Keepalive as u8,
        }
    }
}

impl Frame<&[u8]> {
    pub fn read(mut buffer: &[u8], category: u8) -> Result<Frame<&[u8]>, NetworkError> {
        match category {
            1 => Ok(Frame::ConnectionClosed(buffer.read_u64::<BigEndian>()?)),
            2 => Ok(Frame::Payload(buffer)),
            3 => Ok(Frame::Keepalive(buffer.read_u64::<BigEndian>()?)),
            _ => Err(NetworkError::Fatal(ErrorType::IncorrectCategory)),
        }
    }
}

impl<P: Serialize> Frame<P> {
    pub fn write<W: SizedWrite>(self, stream: &mut W) -> Result<(), NetworkError> {
        match self {
            Frame::ConnectionAccepted(user_id) => stream.write_u64::<BigEndian>(user_id)?,
            Frame::ConnectionClosed(user_id) => stream.write_u64::<BigEndian>(user_id)?,
            Frame::Payload(payload) => payload.serialize(stream)?,
            Frame::Keepalive(user_id) => stream.write_u64::<BigEndian>(user_id)?,
        }
        Ok(())
    }
}

/// Zero sized helper struct for easily constructing control frames. Not intended
/// for sending payloads.
pub struct NoPayload;

impl Serialize for NoPayload {
    fn serialize<W: io::Write>(&self, _stream: &mut W) -> Result<(), NetworkError> {
        panic!("NoPayload is a utility for sending control messages")
    }
}
