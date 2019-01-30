use chrono;
use flux::session::server::SessionKey;
use flux::session::user::PrivateData;
use flux::crypto;
use flux::time::timestamp_secs;
use hashbrown::HashMap;
use serde_derive::{Deserialize, Serialize};
use std::sync::atomic::{AtomicU64, Ordering, ATOMIC_U64_INIT};

/// Simple authenticator that constructs connection tokens based on client supplied serial keys.
pub struct Authenticator {
    sequence: AtomicU64,
    session_key: SessionKey,
    user_info: HashMap<String, UserInfo>,
}

impl Authenticator {
    pub fn new(config: Config, user_info: HashMap<String, UserInfo>) -> Authenticator {
        Authenticator {
            sequence: ATOMIC_U64_INIT,
            session_key: config.session_key,
            user_info,
        }
    }

    /// Authenticate the provided serial key and return a `ConnectionToken` upon success.
    /// The key must exist and there must not be an active ban on it.
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

    /// Returns a snapshot copy of the current user information mapping.
    pub fn snapshot(&self) -> HashMap<String, UserInfo> {
        self.user_info.clone()
    }

    /// Creates a connection token based on the provided `UserInfo` object.
    fn create_token(&self, user: &UserInfo) -> ConnectionToken {
        // Temporary container for storing the private data
        let mut data = PrivateData {
            user_id: user.id,
            client_key: [0u8; 32],
            server_key: [0u8; 32],
        };

        // Generate a random key pair
        crypto::random_bytes(&mut data.client_key);
        crypto::random_bytes(&mut data.server_key);

        let mut private_data = [0u8; PrivateData::SIZE];

        // Write the private data into a byte buffer.
        data.write(&mut private_data[..]).unwrap();

        let mut token = ConnectionToken {
            version: flux::VERSION_ID,
            protocol: flux::PROTOCOL_ID,
            expires: timestamp_secs() + flux::CONNECTION_TOKEN_EXPIRY_SECS,
            sequence: self.sequence.fetch_add(1, Ordering::Relaxed),
            server_key: data.server_key,
            client_key: data.client_key,
            data: [0u8; PrivateData::SIZE + crypto::MAC_SIZE],
        };

        // Construct the additional data for the encryption.
        let aed =
            PrivateData::additional_data(&flux::VERSION_ID[..], flux::PROTOCOL_ID, token.expires).unwrap();

        // Encrypt the private data into the relevant field in the token.
        crypto::encrypt(
            &mut token.data[..],
            &private_data[..],
            &aed[..],
            token.sequence,
            &self.session_key,
        );

        token
    }
}

unsafe impl Send for Authenticator {}
unsafe impl Sync for Authenticator {}

#[derive(Serialize, Deserialize)]
pub struct Config {
    pub session_key: SessionKey,
}

/// Connection token for delivery to the client. The token should be transmitted on secure protocols
/// as it contains sensitive information.
#[derive(Serialize)]
pub struct ConnectionToken {
    pub version: [u8; 16],
    pub protocol: u16,
    pub expires: u64,
    pub sequence: u64,
    pub server_key: [u8; 32],
    pub client_key: [u8; 32],
    #[serde(serialize_with = "<[_]>::serialize")]
    pub data: [u8; PrivateData::SIZE + crypto::MAC_SIZE],
}

#[derive(Serialize, Deserialize, Clone)]
pub struct Note {
    pub text: String,
    pub created: chrono::DateTime<chrono::Utc>,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct Ban {
    pub created: chrono::DateTime<chrono::Utc>,
    pub expiry: Option<chrono::DateTime<chrono::Utc>>,
    pub reason: String,
}

#[derive(Serialize, Deserialize, Clone)]
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

#[derive(Serialize, Debug)]
pub enum AuthError<'a> {
    Failed,
    Banned(&'a Ban),
}
