use crate::net::result::{Error, Result};
use crate::net::shared;

/// Frame containing serialized data.
pub struct Frame<'a> {
    pub category: Category,
    pub data: &'a [u8],
}

/// Message category.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum Category {
    ConnectionAccepted = 0,
    ConnectionClosed = 1,
    Payload = 2,
}

impl Category {
    /// Convert a byte to a category.
    pub fn from_byte(byte: u8) -> Result<Category> {
        match byte {
            0 => Ok(Category::ConnectionAccepted),
            1 => Ok(Category::ConnectionClosed),
            2 => Ok(Category::Payload),
            _ => Err(Error::IncorrectCategory),
        }
    }
}

impl From<Category> for u8 {
    fn from(cls: Category) -> Self {
        cls as u8
    }
}
