#![allow(clippy::len_without_is_empty)]
#![allow(clippy::new_without_default)]
#![allow(clippy::new_without_default_derive)]
pub const PROTOCOL_ID: u16 = 0x0a55;
pub const VERSION_ID: [u8; 16] = *b"NOB_VON_PEN_ISLE";

pub const CONNECTION_TOKEN_EXPIRY_SECS: u64 = 10;

pub type UserId = u64;

pub mod contract;
pub mod crypto;
pub mod time;