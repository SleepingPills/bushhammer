use std::env;
use std::fs;
use std::path::Path;

const GAME_CFG_NAME: &str = "game_config.toml";
const LOG_CFG_NAME: &str = "gamerunner.log.toml";

fn main() {
    let source_dir = env::var("CARGO_MANIFEST_DIR").unwrap();
    let out_dir = env::var("OUT_DIR").unwrap();

    let source_path = Path::new(&source_dir).join("config");

    // Navigate three levels up...
    let out_path = Path::new(&out_dir)
        .parent()
        .and_then(|pth| pth.parent())
        .and_then(|pth| pth.parent())
        .expect("Failed navigating to the target directory");

    fs::copy(source_path.join(GAME_CFG_NAME), out_path.join(GAME_CFG_NAME))
        .expect(&format!("Failed to copy {}", GAME_CFG_NAME));

    fs::copy(source_path.join(LOG_CFG_NAME), out_path.join(LOG_CFG_NAME))
        .expect(&format!("Failed to copy {}", LOG_CFG_NAME));
}
