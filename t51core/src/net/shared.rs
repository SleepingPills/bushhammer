use std::io;
use std::time::SystemTime;

pub type ClientId = u64;

pub trait Serializable: Sized {
    fn serialize<W: io::Write>(&self, stream: &mut W) -> io::Result<()>;
    fn deserialize<R: io::Read>(stream: &mut R) -> io::Result<Self>;
}

/// Returns the current unix timestamp (seconds elapsed since 1970-01-01)
#[inline]
pub fn current_timestamp() -> u64 {
    SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .expect("Closed timelike curve, reality compromised")
        .as_secs()
}
