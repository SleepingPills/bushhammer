use authenticator::{Ban, UserInfo};
use chrono;
use serde_json;
use std::collections::HashMap;

use neutronium::prelude::EntityId;

fn main() {
    let mut infos = HashMap::new();
    infos.insert(b"123123", 1);
    infos.insert(b"222222", 2);

    // Serialize it to a JSON string.
    let j = serde_json::to_string(&infos).unwrap();

    println!("{:?}", EntityId::from(50));

    // Print, write to a file, or send to an HTTP server.
    println!("{}", j);
}
