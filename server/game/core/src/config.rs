use flux::session::server::SessionKey;
use serde_derive::{Deserialize, Serialize};
use serdeconv;
use std::path::Path;

pub const DEFAULT_PORT: u16 = 28008;

#[derive(Serialize, Deserialize)]
pub struct Server {
    pub address: String,
    pub token: SessionKey,
    pub max_clients: u16,
    pub threads: u16,
}

#[derive(Serialize, Deserialize)]
pub struct Game {
    pub fps: u64,
}

#[derive(Serialize, Deserialize)]
pub struct GameConfig {
    pub server: Server,
    pub game: Game,
}

impl GameConfig {
    pub fn load<P: AsRef<Path>>(path: P) -> GameConfig {
        serdeconv::from_toml_file(path).expect("Error loading game configuration file")
    }
}

impl Default for GameConfig {
    fn default() -> GameConfig {
        GameConfig {
            server: Server {
                address: format!("localhost:{}", DEFAULT_PORT),
                token: SessionKey::new([0; SessionKey::SIZE]),
                max_clients: 256,
                threads: 8,
            },
            game: Game { fps: 20 },
        }
    }
}
