use std::time::SystemTime;

/// Returns the current unix timestamp (seconds elapsed since 1970-01-01)
#[inline]
pub fn timestamp_secs() -> u64 {
    SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .expect("Closed timelike curve, reality compromised")
        .as_secs()
}
