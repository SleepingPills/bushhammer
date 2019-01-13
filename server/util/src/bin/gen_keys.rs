use clap::{Arg, App};
use chrono;
use serde_json;
use std::collections::HashMap;
use authenticator::{ClientInfo, Ban};


fn main() {
    let mut infos = HashMap::new();
    infos.insert("1", ClientInfo { ban: None });
    infos.insert(
        "2",
        ClientInfo {
            ban: Some(Ban {
                created: chrono::Utc::now(),
                expired: chrono::Utc::now() + chrono::Duration::days(30),
                reason: "bla".into(),
            }),
        },
    );

    // Serialize it to a JSON string.
    let j = serde_json::to_string(&infos).unwrap();

    // Print, write to a file, or send to an HTTP server.
    println!("{}", j);
}
