use chrono;
use hashbrown::HashMap;
use serde_derive::{Deserialize, Serialize};
use serde_json;
use std::fs;

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
    user_info: HashMap<[u8; 24], UserInfo>,
}

impl Authenticator {
    pub fn new(secret_key: [u8; 32]) -> Authenticator {
        Authenticator {
            secret_key,
            user_info: HashMap::new(),
        }
    }

    pub fn read_config(&mut self, config_path: &str) {
        let config_file = fs::File::open(config_path).unwrap();
        self.user_info = serde_json::from_reader(config_file).unwrap();
    }

    pub fn write_config(&self, config_path: &str) {
        let config_file = fs::File::create(config_path).unwrap();
        serde_json::to_writer_pretty(config_file, &self.user_info).unwrap();
    }
}
