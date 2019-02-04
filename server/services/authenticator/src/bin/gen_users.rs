#![feature(const_slice_len)]

use authenticator::core::UserInfo;
use clap::{App, Arg};
use rand::distributions::Uniform;
use rand::prelude::*;
use serdeconv;
use std::collections::HashMap;

const ALLOWED_CHARS: [char; 35] = [
    '1', '2', '3', '4', '5', '6', '7', '8', '9', 'a', 'b', 'c', 'd', 'e', 'f', 'g', 'h', 'i', 'j', 'k', 'l',
    'm', 'n', 'o', 'p', 'q', 'r', 's', 't', 'u', 'v', 'w', 'x', 'y', 'z',
];

const RANGE: usize = ALLOWED_CHARS.len();
const KEY_LEN: usize = 24;

fn make_key(rng: &mut ThreadRng) -> String {
    rng.sample_iter(&Uniform::new(0, RANGE))
        .take(KEY_LEN)
        .map(|sample| ALLOWED_CHARS[sample])
        .collect()
}

fn main() {
    let matches = App::new("Key Generator")
        .version("1.0")
        .author("Bush Hammer Industries")
        .about("Generates user serial key entries.")
        .arg(
            Arg::with_name("USER_FILE")
                .help("Path to the client file")
                .required(true),
        )
        .arg(
            Arg::with_name("NKEYS")
                .help("Number of new keys to generate")
                .required(true),
        )
        .arg(
            Arg::with_name("KEY_FILE")
                .help("Newly generated keys will be written in this file")
                .required(false),
        )
        .get_matches();

    let user_file_path = matches.value_of("USER_FILE").unwrap();
    let key_count: usize = matches
        .value_of("NKEYS")
        .unwrap()
        .parse()
        .expect("Key count must be a valid integer");

    let mut user_data: HashMap<String, UserInfo> = serdeconv::from_toml_file(user_file_path).unwrap();
    let mut rng = thread_rng();
    let mut keys = Vec::new();

    let id_base = user_data.len() as u64;

    println!("Current client data contains {} entries", user_data.len());
    println!("Generating {} keys", key_count);
    for i in 0..key_count as u64 {
        let key = make_key(&mut rng);
        keys.push(key.clone());
        user_data
            .entry(key)
            .and_modify(|_| panic!("Key collision! What are the odds?"))
            .or_insert_with(|| UserInfo::new(id_base + i));
    }

    serdeconv::to_toml_file(&user_data, user_file_path).unwrap();

    if let Some(key_file_path) = matches.value_of("KEY_FILE") {
        println!("Writing keys to key file `{}`", key_file_path);
        serdeconv::to_toml_file(&keys, key_file_path).unwrap();
    }
}
