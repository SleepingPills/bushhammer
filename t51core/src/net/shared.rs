use crate::net::result::Result;
use std::io;
use std::time::SystemTime;

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

/// Returns the current unix timestamp (seconds elapsed since 1970-01-01)
#[inline]
pub fn current_timestamp() -> u64 {
    SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .expect("Closed timelike curve, reality compromised")
        .as_secs()
}
