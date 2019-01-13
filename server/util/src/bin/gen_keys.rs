use authenticator::{Ban, ClientInfo};
use chrono;
use clap::{App, Arg};
use rand::prelude::*;
use serde_json;
use std::collections::HashMap;
use std::fs;

fn main() {
    let matches = App::new("Key Generator")
        .version("1.0")
        .author("Bush Hammer Industries")
        .about("Generates client and serial key entries.")
        .arg(
            Arg::with_name("CLIENT_FILE")
                .help("Path to the client file")
                .required(true),
        )
        .arg(
            Arg::with_name("KEY_FILE")
                .help("Newly generated keys will be written in this file")
                .required(true),
        )
        .arg(
            Arg::with_name("NKEYS")
                .help("Number of new keys to generate")
                .required(true),
        )
        .get_matches();
    let client_file_path = matches.value_of("CLIENT_FILE").unwrap();

//    let mut client_file = fs::OpenOptions::new()
//        .create(true)
//        .read(true)
//        .open(client_file_path)
//        .unwrap();

    let client_data = match fs::read_to_string(client_file_path) {
        Ok(content) => content,
        Err(_) => "".into(),
    };

    let client_data: HashMap<[u8; 24], ClientInfo> = serde_json::from_str(&client_data).unwrap();
    //    let mut infos = HashMap::new();
    //    infos.insert("1", ClientInfo { ban: None });
    //    infos.insert(
    //        "2",
    //        ClientInfo {
    //            ban: Some(Ban {
    //                created: chrono::Utc::now(),
    //                expired: chrono::Utc::now() + chrono::Duration::days(30),
    //                reason: "bla".into(),
    //            }),
    //        },
    //    );
    //
    //    // Serialize it to a JSON string.
    //    let j = serde_json::to_string(&infos).unwrap();
    //
    //    // Print, write to a file, or send to an HTTP server.
    //    println!("{}", j);
    // let mut rng = thread_rng();
}
