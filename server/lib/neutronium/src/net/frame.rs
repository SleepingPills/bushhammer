use crate::net::support::{ErrorType, NetworkError, SizedWrite};
use byteorder::{BigEndian, ReadBytesExt, WriteBytesExt};
use flux::UserId;

#[derive(Debug, Eq, PartialEq)]
pub enum Category {
    Payload = 0,
    Keepalive = 1,
    ConnectionAccepted = 2,
    ConnectionClosed = 3,
}

impl From<Category> for u8 {
    #[inline]
    fn from(cat: Category) -> Self {
        cat as u8
    }
}

#[derive(Debug, Eq, PartialEq)]
#[repr(transparent)]
pub struct PayloadInfo(usize);

impl PayloadInfo {
    /// Selects the correct slice of the payload buffer
    #[inline]
    pub(crate) fn select(self, payload: &[u8]) -> &[u8] {
        &payload[..self.0]
    }
}

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
    #[inline]
    pub fn read(mut buffer: &[u8], category: u8) -> Result<Frame, NetworkError> {
        if category > Category::ConnectionClosed.into() {
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
    #[inline]
    pub fn category(&self) -> Category {
        match self {
            ControlFrame::Keepalive(_) => Category::Keepalive,
            ControlFrame::ConnectionAccepted(_) => Category::ConnectionAccepted,
            ControlFrame::ConnectionClosed(_) => Category::ConnectionClosed,
        }
    }

    #[inline]
    pub fn write<W: SizedWrite>(self, stream: &mut W) -> Result<(), NetworkError> {
        match self {
            ControlFrame::Keepalive(user_id) => stream.write_u64::<BigEndian>(user_id)?,
            ControlFrame::ConnectionAccepted(user_id) => stream.write_u64::<BigEndian>(user_id)?,
            ControlFrame::ConnectionClosed(user_id) => stream.write_u64::<BigEndian>(user_id)?,
        }
        Ok(())
    }
}
