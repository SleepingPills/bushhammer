#![feature(proc_macro_hygiene, decl_macro)]
use authenticator::core::{Authenticator, Config, UserInfo, ConnectionToken, AuthError};
use clap::{App, Arg};
use hashbrown::HashMap;
use rocket;
use rocket::{post, routes, State};
use rocket_contrib::json::{Json};
use serde_json;
use std::fs;

#[post("/auth", data="<auth_key>")]
fn auth(auth: State<Authenticator>, auth_key: String) -> Result<Json<ConnectionToken>, AuthError> {
    auth.authenticate(auth_key).map(|token| Json(token))
}

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
            Arg::with_name("USER_FILE")
                .help("Path to the client file")
                .required(true),
        )
        .get_matches();

    let config_file_path = matches.value_of("CONFIG_FILE").unwrap();
    let client_file_path = matches.value_of("USER_FILE").unwrap();

    let config: Config =
        serde_json::from_reader(fs::File::open(config_file_path).expect("Error opening config file"))
            .expect("Error parsing config file");
    let user_info: HashMap<String, UserInfo> =
        serde_json::from_reader(fs::File::open(client_file_path).expect("Error opening client data file"))
            .expect("Error parsing client data file");

    let authenticator = Authenticator::new(config, user_info);

    rocket::ignite()
        .mount("/user", routes![auth])
        .manage(authenticator)
        .launch();
}
