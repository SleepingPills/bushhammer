#![feature(integer_atomics)]

use chrono;
use flux::contract::PrivateData;
use flux::crypto;
use flux::time::timestamp_secs;
use hashbrown::HashMap;
use serde_derive::{Deserialize, Serialize};
use serde_json;
use std::fs;
use std::sync::atomic::{AtomicU64, Ordering, ATOMIC_U64_INIT};

#[derive(Serialize, Deserialize)]
struct AuthenticatorConfig {
    secret_key: Vec<u8>,
    server_address: String,
    user_info: HashMap<String, UserInfo>,
}

pub struct Authenticator {
    sequence: AtomicU64,
    secret_key: Vec<u8>,
    server_address: String,
    user_info: HashMap<String, UserInfo>,
}

impl Authenticator {
    pub fn new(config_path: String) -> Authenticator {
        let config_file = fs::File::open(config_path).unwrap();
        let config: AuthenticatorConfig = serde_json::from_reader(config_file).unwrap();

        assert_eq!(config.secret_key.len(), flux::SECRET_KEY_SIZE);
        assert!(config.server_address.len() < 253);

        Authenticator {
            sequence: ATOMIC_U64_INIT,
            secret_key: config.secret_key,
            server_address: config.server_address,
            user_info: config.user_info,
        }
    }

    pub fn authenticate(&self, serial_key: String) -> Result<ConnectionToken, AuthError> {
        match self.user_info.get(&serial_key) {
            Some(info) => {
                if let Some(ban) = &info.ban {
                    return Err(AuthError::Banned(ban));
                }

                Ok(self.create_token(info))
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

        let mut private_data = [0u8; PrivateData::SIZE + crypto::MAC_SIZE];

        data.write(&mut private_data[..]).unwrap();

        /*
        1. Generate expiry timestamp
        2. Bump sequence and pass it in
        3. Move in keys
        4. Create empty data array in the token and encrypt into it
        */
        let token = ConnectionToken {
            version: flux::VERSION_ID,
            protocol: flux::PROTOCOL_ID,
            expires: timestamp_secs() + flux::CONNECTION_TOKEN_EXPIRY_SECS,
            sequence: self.sequence.fetch_add(1, Ordering::Relaxed),
            server_key: data.server_key,
            client_key: data.client_key,
            server_address: &self.server_address,
            data: [0u8; PrivateData::SIZE + crypto::MAC_SIZE],
        };

        let aed = PrivateData::additional_data(&flux::VERSION_ID[..], flux::PROTOCOL_ID, token.expires).unwrap();

        crypto::encrypt(&mut token.data[..], &data[..], &aed[..], token.sequence, &self.secret_key);

        token
    }
}

#[derive(Serialize)]
pub struct ConnectionToken<'a> {
    pub version: [u8; 16],
    pub protocol: u16,
    pub expires: u64,
    pub sequence: u64,
    pub server_key: [u8; 32],
    pub client_key: [u8; 32],
    pub server_address: &'a str,
    #[serde(serialize_with = "<[_]>::serialize")]
    pub data: [u8; PrivateData::SIZE + crypto::MAC_SIZE],
}

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
pub enum AuthError<'a> {
    Failed,
    Banned(&'a Ban),
}
