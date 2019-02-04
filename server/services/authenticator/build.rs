use std::env;
use std::fs;
use std::path::Path;

const ROCKET_CFG_NAME: &str = "rocket.toml";
const AUTH_CFG_NAME: &str = "authenticator.log.toml";

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

    fs::copy(source_path.join(ROCKET_CFG_NAME), out_path.join(ROCKET_CFG_NAME))
        .expect(&format!("Failed to copy {}", ROCKET_CFG_NAME));

    fs::copy(source_path.join(AUTH_CFG_NAME), out_path.join(AUTH_CFG_NAME))
        .expect(&format!("Failed to copy {}", AUTH_CFG_NAME));
}
