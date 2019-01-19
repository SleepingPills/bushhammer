#![feature(integer_atomics)]

use chrono;
use flux::contract::PrivateData;
use flux::crypto;
use hashbrown::HashMap;
use serde_derive::{Deserialize, Serialize};
use serde_json;
use std::fs;
use std::sync::atomic::{AtomicU64, ATOMIC_U64_INIT};

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

#[derive(Serialize)]
pub struct ConnectionToken {
    pub version: [u8; 16],
    pub protocol: u16,
    pub expires: u64,
    pub sequence: u64,
    pub server_key: [u8; 32],
    pub client_key: [u8; 32],
    #[serde(serialize_with = "<[_]>::serialize")]
    pub server_address: [u8; 256],
    #[serde(serialize_with = "<[_]>::serialize")]
    pub data: [u8; 72],
}

#[derive(Serialize)]
pub enum AuthError<'a> {
    Failed,
    Banned(&'a Ban),
}

pub struct Authenticator {
    sequence: AtomicU64,
    config_path: String,
    secret_key: [u8; 32],
    user_info: HashMap<String, UserInfo>,
}

impl Authenticator {
    pub fn new(config_path: String, secret_key: [u8; 32]) -> Authenticator {
        Authenticator {
            sequence: ATOMIC_U64_INIT,
            config_path,
            secret_key,
            user_info: HashMap::new(),
        }
    }

    pub fn read_config(&mut self) {
        let config_file = fs::File::open(&self.config_path).unwrap();
        self.user_info = serde_json::from_reader(config_file).unwrap();
    }

    pub fn authenticate(&self, serial_key: String) -> Result<ConnectionToken, AuthError> {
        match self.user_info.get(&serial_key) {
            Some(info) => {
                if let Some(ban) = &info.ban {
                    return Err(AuthError::Banned(ban));
                }

                unimplemented!()
            }
            None => Err(AuthError::Failed),
        }
    }

    fn create_token(&self, user: &UserInfo) -> ConnectionToken {
        let mut data = PrivateData {
            user_id: user.id,
            client_key: [0u8; 32],
            server_key: [0u8; 32],
        };

        crypto::random_bytes(&mut data.client_key);
        crypto::random_bytes(&mut data.server_key);

        let mut private_data = [0u8; PrivateData::SIZE];

        data.write(&mut private_data[..]).unwrap();

        

        unimplemented!()
    }
}
