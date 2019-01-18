#![allow(clippy::len_without_is_empty)]
#![allow(clippy::new_without_default)]
#![allow(clippy::new_without_default_derive)]

pub const PROTOCOL_ID: u16 = 0x0a55;
pub const VERSION_ID: [u8; 16] = [0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0];

pub type UserId = u64;

pub mod crypto;
pub mod contract;