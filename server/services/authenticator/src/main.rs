#![feature(proc_macro_hygiene, decl_macro)]
use authenticator::core::{AuthResult, Authenticator, Config, UserInfo};
use clap::{App, Arg};
use flux::logging;
use hashbrown::HashMap;
use rocket;
use rocket::{post, routes, State};
use rocket_contrib::json::Json;
use serdeconv;

#[post("/auth", data = "<auth_key>")]
fn auth(auth: State<Authenticator>, auth_key: String) -> Json<AuthResult> {
    Json(auth.authenticate(auth_key))
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

    // Initialize logging
    let logger = logging::init();

    let config_file_path = matches.value_of("CONFIG_FILE").unwrap();
    logging::debug!(logger, "setup"; "config_file_path" => config_file_path);

    let client_file_path = matches.value_of("USER_FILE").unwrap();
    logging::debug!(logger, "setup"; "client_file_path" => config_file_path);

    let config: Config = serdeconv::from_toml_file(config_file_path).expect("Error parsing config file");
    let user_info: HashMap<String, UserInfo> =
        serdeconv::from_toml_file(client_file_path).expect("Error parsing client data file");

    logging::info!(logger, "starting web server");
    // Create rocket instnace
    let rocket_instance = rocket::ignite()
        .mount("/user", routes![auth])
        .manage(Authenticator::new(config, user_info, &logger));

    let cfg = rocket_instance.config();

    logging::info!(logger, "rocket config";
                   "environment" => cfg.environment.to_string(),
                   "address" => &cfg.address,
                   "port" => cfg.port,
                   "workers" => cfg.workers,
                   "keep_alive" => cfg.keep_alive,
                   "log_level" => cfg.log_level.to_string());

    rocket_instance.launch();
}
