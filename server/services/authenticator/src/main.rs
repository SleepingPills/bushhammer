use authenticator::core::{Authenticator, UserInfo, Config};
use clap::{App, Arg};
use hashbrown::HashMap;
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
        .arg(
            Arg::with_name("NTHREADS")
                .help("Number of threads for handling requests")
                .default_value("2")
        )
        .get_matches();

    let config_file_path = matches.value_of("CONFIG_FILE").unwrap();
    let client_file_path = matches.value_of("CLIENT_FILE").unwrap();

    let nthreads: usize = matches
        .value_of("NTHREADS")
        .unwrap()
        .parse()
        .expect("Thread count must be a valid integer");

    let config: Config =
        serde_json::from_reader(fs::File::open(config_file_path).expect("Error opening config file"))
            .expect("Error parsing config file");
    let user_info: HashMap<String, UserInfo> =
        serde_json::from_reader(fs::File::open(client_file_path).expect("Error opening client data file"))
            .expect("Error parsing client data file");

    let authenticator = Authenticator::new(config, user_info);
}
