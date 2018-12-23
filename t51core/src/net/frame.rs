use serde_derive::{Deserialize, Serialize};
use std::time::SystemTime;

/*
[version info] (13 bytes)       // "NETCODE 1.02" ASCII with null terminator.
[protocol id] (uint64)          // 64 bit value unique to this particular game/application
[create timestamp] (uint64)     // 64 bit unix timestamp when this connect token was created
[expire timestamp] (uint64)     // 64 bit unix timestamp when this connect token expires
[connect token nonce] (24 bytes)
*/
#[derive(Serialize, Deserialize)]
pub struct ConnectToken {
    pub version: [u8; 16],
    pub protocol: u64,
    pub created: SystemTime,
    pub expires: SystemTime,
}

#[derive(Serialize, Deserialize)]
pub struct PrivateData {
    pub client_id: u64,
    pub server_key: [u8; 32],
    pub client_key: [u8; 32],
}

#[derive(Serialize, Deserialize)]
pub struct Header {
    pub class: u8,
    pub sequence: u64,
    pub size: u16,
}

pub struct Frame {
    pub header: Header,
    pub data: Vec<u8>,
}
