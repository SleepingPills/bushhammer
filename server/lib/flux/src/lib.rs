#![allow(clippy::len_without_is_empty)]
#![allow(clippy::new_without_default)]
#![allow(clippy::new_without_default_derive)]
pub const PROTOCOL_ID: u16 = 0x0a55;
pub const VERSION_ID: [u8; 16] = *b"NOB_VON_PEN_ISLE";

pub const CONNECTION_TOKEN_EXPIRY_SECS: u64 = 10;

pub type UserId = u64;

pub mod crypto;
pub mod logging;
pub mod session;
pub mod time;
pub mod util;

pub mod encoding {
    pub mod base64 {
        pub use base64::{decode, encode};
        use serde::{de, Deserialize, Deserializer, Serializer};

        #[inline]
        pub fn serialize<S>(bytes: &[u8], serializer: S) -> Result<S::Ok, S::Error>
        where
            S: Serializer,
        {
            serializer.serialize_str(&encode(bytes))
        }

        #[inline]
        pub fn deserialize<'de, D>(deserializer: D) -> Result<Vec<u8>, D::Error>
        where
            D: Deserializer<'de>,
        {
            let s = <&str>::deserialize(deserializer)?;
            decode(s).map_err(de::Error::custom)
        }
    }
}
