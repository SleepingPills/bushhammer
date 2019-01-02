use crate::net::result::{Error, Result};
use crate::net::shared::{UserId, Serialize, SizedWrite};
use byteorder::{BigEndian, ReadBytesExt, WriteBytesExt};
use std::io;

pub enum Frame<P> {
    ConnectionAccepted(UserId),
    ConnectionClosed(UserId),
    Payload(P),
    Keepalive(UserId),
}

impl<P> Frame<P> {
    pub fn category(&self) -> u8 {
        match self {
            Frame::ConnectionAccepted(_) => 0,
            Frame::ConnectionClosed(_) => 1,
            Frame::Payload(_) => 2,
            Frame::Keepalive(_) => 3,
        }
    }
}

impl Frame<&[u8]> {
    pub fn read(mut buffer: &[u8], category: u8) -> Result<Frame<&[u8]>> {
        match category {
            1 => Ok(Frame::ConnectionClosed(buffer.read_u64::<BigEndian>()?)),
            2 => Ok(Frame::Payload(buffer)),
            3 => Ok(Frame::Keepalive(buffer.read_u64::<BigEndian>()?)),
            _ => Err(Error::IncorrectCategory),
        }
    }
}

impl<P: Serialize> Frame<P> {
    pub fn write<W: SizedWrite>(mut self, stream: &mut W) -> Result<()> {
        match self {
            Frame::ConnectionAccepted(user_id) => stream.write_u64::<BigEndian>(user_id)?,
            Frame::ConnectionClosed(user_id) => stream.write_u64::<BigEndian>(user_id)?,
            Frame::Payload(ref mut payload) => payload.serialize(stream)?,
            Frame::Keepalive(user_id) => stream.write_u64::<BigEndian>(user_id)?,
        }
        Ok(())
    }
}

/// Zero sized helper struct for easily constructing control frames. Not intended
/// for sending payloads.
pub struct NoPayload;

impl Serialize for NoPayload {
    fn serialize<W: io::Write>(&mut self, _stream: &mut W) -> Result<()> {
        panic!("NoPayload is only a utility for sending control messages")
    }
}

/// Shorthand for constructing control frames with no payload.
pub type ControlFrame = Frame<NoPayload>;