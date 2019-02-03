use std::env;
use std::fs;
use std::path::Path;

fn main() {
    let source_dir = env::var("CARGO_MANIFEST_DIR").unwrap();
    let out_dir = env::var("OUT_DIR").unwrap();

    let source_path = Path::new(&source_dir).join("config");
    let out_path = Path::new(&out_dir);

    fs::copy(source_path.join("rocket.toml"), out_path.join("rocket.toml"));
    fs::copy(source_path.join("authenticator.log.toml"), out_path.join("authenticator.log.toml"));
}