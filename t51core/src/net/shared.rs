use std::io;

pub type ClientId = u64;

pub trait Serializable : Sized {
    fn serialize<W: io::Write>(&self, stream: &mut W) -> io::Result<()>;
    fn deserialize<R: io::Read>(stream: &mut R) -> io::Result<Self>;
}