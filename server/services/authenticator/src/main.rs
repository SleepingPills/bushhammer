//use authenticator::core::AuthenticatorConfig;
use clap::{App, Arg};
use serde_derive::{Deserialize, Serialize};
use serde_json;

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
        .arg(
            Arg::with_name("NTHREADS")
                .help("Number of new keys to generate")
                .default_value("2"),
        )
        .get_matches();

    let config_file_path = matches.value_of("CONFIG_FILE").unwrap();
    let client_file_path = matches.value_of("CLIENT_FILE").unwrap();
    let thread_count = matches.value_of("NTHREADS").unwrap();
}

#[derive(Serialize, Deserialize)]
struct Config {
    secret_key: [u8; flux::SECRET_KEY_SIZE],
    server_address: String,
}
