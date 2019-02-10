use serde_derive::{Deserialize, Serialize};
use serdeconv;
use std::path::Path;

#[derive(Serialize, Deserialize)]
pub struct GameConfig {
    pub address: Option<String>,
}

impl GameConfig {
    pub fn load<P: AsRef<Path>>(path: P) -> GameConfig {
        serdeconv::from_toml_file(path).expect("Error loading game configuration file")
    }
}
