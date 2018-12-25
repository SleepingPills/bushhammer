use serde_derive::{Deserialize, Serialize};

#[derive(Serialize, Deserialize)]
pub struct ConnectionHeader {
    pub version: [u8; 16],
    pub protocol: u64,
    pub created: u64,
    pub expires: u64,
    pub sequence: u64,
}

#[derive(Serialize, Deserialize)]
pub struct PrivateData {
    pub client_id: u64,
    pub server_key: [u8; 32],
    pub client_key: [u8; 32],
}

#[derive(Serialize, Deserialize)]
pub struct ChallengeHeader {
    pub sequence: u64,
}

#[derive(Serialize, Deserialize)]
pub struct PayloadHeader {
    pub class: u8,
    pub sequence: u64,
    pub size: u16,
}
pub struct Frame {
    pub header: PayloadHeader,
    pub data: Vec<u8>,
}
