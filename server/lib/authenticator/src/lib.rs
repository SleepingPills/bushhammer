use chrono;
use hashbrown::HashMap;
use serde_derive::{Deserialize, Serialize};
use serde_json;

#[derive(Serialize, Deserialize)]
pub struct Ban {
    pub created: chrono::DateTime<chrono::Utc>,
    pub expired: chrono::DateTime<chrono::Utc>,
    pub reason: String,
}

#[derive(Serialize, Deserialize)]
pub struct ClientInfo {
    pub ban: Option<Ban>,
}

pub struct Authenticator {
    secret_key: [u8; 32],
    serial_numbers: HashMap<[u8; 24], ClientInfo>,
}

impl Authenticator {
    pub fn from_file() {
        let a = Ban {
            created: chrono::Utc::now(),
            expired: chrono::Utc::now() + chrono::Duration::days(30),
            reason: "blablabla".into(),
        };
    }
}
