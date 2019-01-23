use authenticator::core::Config;
use clap::{App, Arg};
use flux::crypto;
use serde_json;
use std::fs;

fn main() {
    let matches = App::new("Key Generator")
        .version("1.0")
        .author("Bush Hammer Industries")
        .about("Generates client and serial key entries.")
        .arg(
            Arg::with_name("CONFIG_FILE")
                .help("Path to the config file")
                .required(true),
        )
        .get_matches();

    let config_file_path = matches.value_of("CONFIG_FILE").unwrap();

    let mut config = Config {
        secret_key: [0; 32],
    };

    crypto::random_bytes(&mut config.secret_key[..]);

    let key_file = fs::OpenOptions::new()
        .create(true)
        .write(true)
        .truncate(true)
        .open(config_file_path)
        .unwrap();

    serde_json::to_writer_pretty(key_file, &config).expect("Config serialization failed")
}
