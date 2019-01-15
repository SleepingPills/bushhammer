use chrono;
use hashbrown::HashMap;
use serde_derive::{Deserialize, Serialize};
use serde_json;

#[derive(Serialize, Deserialize)]
pub struct Note {
    pub text: String,
    pub created: chrono::DateTime<chrono::Utc>,
}

#[derive(Serialize, Deserialize)]
pub struct Ban {
    pub created: chrono::DateTime<chrono::Utc>,
    pub expiry: Option<chrono::DateTime<chrono::Utc>>,
    pub reason: String,
}

#[derive(Serialize, Deserialize)]
pub struct UserInfo {
    pub id: u64,
    pub created: chrono::DateTime<chrono::Utc>,
    pub notes: Vec<Note>,
    pub ban: Option<Ban>,
}

impl UserInfo {
    pub fn new(id: u64) -> UserInfo {
        UserInfo {
            id,
            created: chrono::Utc::now(),
            notes: Vec::new(),
            ban: None,
        }
    }
}

pub struct Authenticator {
    secret_key: [u8; 32],
    serial_numbers: HashMap<[u8; 24], UserInfo>,
}

impl Authenticator {
    pub fn from_file() {}
}
