use crate::net::shared::{ErrorType, NetworkError, NetworkResult, Serialize, SizedWrite, UserId};
use byteorder::{BigEndian, ReadBytesExt, WriteBytesExt};
use std::io;

pub enum Category {
    Payload = 0,
    Keepalive = 1,
    ConnectionAccepted = 2,
    ConnectionClosed = 3,
}

impl From<Category> for u8 {
    fn from(cat: Category) -> Self {
        cat as u8
    }
}

#[repr(transparent)]
pub struct PayloadInfo(usize);

#[derive(Debug, Eq, PartialEq)]
pub enum ControlFrame {
    Keepalive(UserId),
    ConnectionAccepted(UserId),
    ConnectionClosed(UserId),
}

#[derive(Debug, Eq, PartialEq)]
pub enum Frame {
    Control(ControlFrame),
    Payload(PayloadInfo),
}

impl Frame {
    pub fn read(mut buffer: &[u8], category: u8) -> Result<Frame, NetworkError> {
        if category > Category::ConnectionClosed {
            return Err(NetworkError::Fatal(ErrorType::IncorrectCategory));
        }

        Ok(match category {
            0 => Frame::Payload(PayloadInfo(buffer.len())),
            1 => Frame::Control(ControlFrame::Keepalive(buffer.read_u64::<BigEndian>()?)),
            2 => Frame::Control(ControlFrame::ConnectionAccepted(buffer.read_u64::<BigEndian>()?)),
            3 => Frame::Control(ControlFrame::ConnectionClosed(buffer.read_u64::<BigEndian>()?)),
            _ => unreachable!(),
        })
    }
}

impl ControlFrame {
    pub fn category(&self) -> u8 {
        match self {
            ControlFrame::Keepalive(_) => Category::Keepalive.into(),
            ControlFrame::ConnectionAccepted(_) => Category::ConnectionAccepted.into(),
            ControlFrame::ConnectionClosed(_) => Category::ConnectionClosed.into(),
        }
    }

    pub fn write<W: SizedWrite>(self, stream: &mut W) -> Result<(), NetworkError> {
        match self {
            Frame::Keepalive(user_id) => stream.write_u64::<BigEndian>(user_id)?,
            Frame::ConnectionAccepted(user_id) => stream.write_u64::<BigEndian>(user_id)?,
            Frame::ConnectionClosed(user_id) => stream.write_u64::<BigEndian>(user_id)?,
        }
        Ok(())
    }
}
