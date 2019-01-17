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

// TODO: Add token sequence check to Endpoint
#[derive(Serialize)]
pub struct ConnectionToken {
    pub version: [u8; 16],
    pub protocol: u16,
    pub expires: u64,
    pub sequence: u64,
    pub data: [u8; 72],
}

#[derive(Serialize)]
pub enum AuthError<'a> {
    Failed,
    Banned(&'a Ban),
}


pub struct Authenticator {
    secret_key: [u8; 32],
    user_info: HashMap<String, UserInfo>,
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

    pub fn authenticate(&self, serial_key: String) -> Result<ConnectionToken, AuthError> {
        match self.user_info.get(&serial_key) {
            Some(info) => {
                if let Some(ban) = &info.ban {
                    return Err(AuthError::Banned(ban))
                }

                unimplemented!()
            },
            None => Err(AuthError::Failed)
        }
    }
}
