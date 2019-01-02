use crate::net::result::Result;
use std::io;

pub type UserId = u64;

pub trait SizedWrite: io::Write {
    fn free_capacity(&self) -> usize;
}

impl SizedWrite for io::Cursor<&mut [u8]> {
    #[inline]
    fn free_capacity(&self) -> usize {
        self.get_ref().len() - self.position() as usize
    }
}

pub trait Serialize {
    fn serialize<W: SizedWrite>(&mut self, stream: &mut W) -> Result<()>;
}

pub trait Deserialize {
    fn deserialize<R: io::Read>(&self, stream: &mut R) -> Result<()>;
}

pub trait DeserializeSelf {
    fn deserialize_self<R: io::Read>(&mut self, stream: &mut R) -> Result<()>;
}
