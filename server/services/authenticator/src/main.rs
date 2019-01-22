use authenticator::core::{Authenticator, UserInfo};
use clap::{App, Arg};
use hashbrown::HashMap;
use serde_derive::{Deserialize, Serialize};
use serde_json;
use std::fs;

pub fn main() {
    let matches = App::new("Authenticator Service")
        .version("1.0")
        .author("Bush Hammer Industries")
        .about("Runs the authenticator server.")
        .arg(
            Arg::with_name("CONFIG_FILE")
                .help("Path to the config file")
                .required(true),
        )
        .arg(
            Arg::with_name("CLIENT_FILE")
                .help("Path to the client file")
                .required(true),
        )
        .get_matches();

    let config_file_path = matches.value_of("CONFIG_FILE").unwrap();
    let client_file_path = matches.value_of("CLIENT_FILE").unwrap();

    let config: Config =
        serde_json::from_reader(fs::File::open(config_file_path).expect("Error opening config file"))
            .expect("Error parsing config file");
    let user_info: HashMap<String, UserInfo> =
        serde_json::from_reader(fs::File::open(client_file_path).expect("Error opening client data file"))
            .expect("Error parsing client data file");

    let authenticator = Authenticator::new(config.secret_key, config.server_address, user_info);
}

#[derive(Serialize, Deserialize)]
struct Config {
    secret_key: [u8; flux::SECRET_KEY_SIZE],
    server_address: String,
    thread_count: usize,
}
