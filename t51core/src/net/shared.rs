use crate::net::result::Result;
use std::io;

pub type ClientId = u64;

pub trait Serialize {
    fn serialize<W: io::Write>(&self, stream: &mut W) -> Result<()>;
}

pub trait Deserialize {
    fn deserialize<R: io::Read>(&self, stream: &mut R) -> Result<()>;
}

pub trait DeserializeSelf {
    fn deserialize_self<R: io::Read>(&mut self, stream: &mut R) -> Result<()>;
}
