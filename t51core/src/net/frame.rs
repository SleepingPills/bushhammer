use crate::net::result::{Error, Result};
use crate::net::shared::{ClientId, Serialize};
use byteorder::{BigEndian, ReadBytesExt, WriteBytesExt};
use std::io;

pub enum Frame<P> {
    ConnectionAccepted(ClientId),
    ConnectionClosed(ClientId),
    Payload(P),
}

impl<P> Frame<P> {
    pub fn category(&self) -> Result<u8> {
        match self {
            Frame::ConnectionAccepted(_) => Ok(0),
            Frame::ConnectionClosed(_) => Ok(1),
            Frame::Payload(_) => Ok(2),
        }
    }
}

impl Frame<&[u8]> {
    pub fn read(mut buffer: &[u8], category: u8) -> Result<Frame<&[u8]>> {
        match category {
            1 => Ok(Frame::ConnectionClosed(buffer.read_u64::<BigEndian>()?)),
            2 => Ok(Frame::Payload(buffer)),
            _ => Err(Error::IncorrectCategory),
        }
    }
}

impl<P: Serialize> Frame<P> {
    pub fn write<W: io::Write>(self, stream: &mut W) -> Result<()> {
        match self {
            Frame::ConnectionAccepted(client_id) => stream.write_u64::<BigEndian>(client_id)?,
            Frame::ConnectionClosed(client_id) => stream.write_u64::<BigEndian>(client_id)?,
            Frame::Payload(payload) => payload.serialize(stream)?,
        }
        Ok(())
    }
}