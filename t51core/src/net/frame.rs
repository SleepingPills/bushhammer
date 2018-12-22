use serde_derive::{Deserialize, Serialize};

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
