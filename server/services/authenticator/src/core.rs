use chrono;
use flux::choose;
use flux::crypto;
use flux::encoding::base64;
use flux::logging;
use flux::session::server::SessionKey;
use flux::session::user::PrivateData;
use flux::time::timestamp_secs;
use hashbrown::HashMap;
use serde_derive::{Deserialize, Serialize};
use std::sync::atomic::{AtomicU64, Ordering, ATOMIC_U64_INIT};

pub const KEY_LEN: usize = 24;

/// Simple authenticator that constructs connection tokens based on client supplied serial keys.
pub struct Authenticator {
    sequence: AtomicU64,
    session_key: SessionKey,
    user_info: HashMap<String, UserInfo>,
    log: logging::Logger,
}

impl Authenticator {
    pub fn new(config: Config, user_info: HashMap<String, UserInfo>, log: &logging::Logger) -> Authenticator {
        Authenticator {
            sequence: ATOMIC_U64_INIT,
            session_key: config.session_key,
            user_info,
            log: log.new(logging::o!()),
        }
    }

    /// Authenticate the provided serial key and return an `AuthResult`.
    /// The key must exist and there must not be an active ban on it.
    pub fn authenticate(&self, serial_key: String) -> AuthResult {
        logging::debug!(self.log, "authentication"; "key" => Self::protect_key(&serial_key));
        match self.user_info.get(&serial_key) {
            Some(info) => {
                if let Some(ban) = &info.ban {
                    let expiry_str = ban.expiry.map_or("N/A".to_string(), |expiry| expiry.to_rfc3339());
                    logging::info!(
                        self.log,
                        "authentication";
                        "result" => "banned",
                        "id" => info.id,
                        "key" => Self::protect_key(&serial_key),
                        "reason" => &ban.reason,
                        "expiry" => &expiry_str
                    );
                    return AuthResult::Banned(ban.clone());
                }

                let token = self.create_token(info);
                logging::info!(
                    self.log,
                    "authentication";
                    "result" => "ok",
                    "id" => info.id,
                    "key" => Self::protect_key(&serial_key),
                    "sequence" => token.sequence,
                    "expiry" => token.expires
                );
                AuthResult::Ok(token)
            }
            None => {
                logging::info!(
                    self.log,
                    "authentication";
                    "result" => "notfound",
                    "key" => Self::protect_key(&serial_key),
                );
                AuthResult::Failed
            }
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

        logging::debug!(self.log, "create token"; "user_id" => user.id, "step" => "generate key pair");
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

        logging::debug!(self.log, "create token"; "user_id" => user.id, "step" => "coalesce aead");
        // Construct the additional data for the encryption.
        let aed =
            PrivateData::additional_data(&flux::VERSION_ID[..], flux::PROTOCOL_ID, token.expires).unwrap();

        logging::debug!(self.log, "create_token"; "user_id" => user.id, "step" => "encrypt private data");
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

    #[inline]
    fn protect_key(serial_key: &String) -> String {
        serial_key
            .chars()
            .enumerate()
            .map(|(idx, chr)| choose!(idx < KEY_LEN - 8 => '*', chr))
            .collect()
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
    #[serde(with = "base64")]
    pub version: [u8; 16],
    pub protocol: u16,
    pub expires: u64,
    pub sequence: u64,
    #[serde(with = "base64")]
    pub server_key: [u8; 32],
    #[serde(with = "base64")]
    pub client_key: [u8; 32],
    #[serde(with = "base64")]
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

#[derive(Serialize)]
#[serde(tag = "result", content = "data")]
pub enum AuthResult {
    Ok(ConnectionToken),
    Failed,
    Banned(Ban),
}
