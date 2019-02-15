use clap::{App, Arg};
use flux::logging;
use gamecore::config::GameConfig;
use gamecore::init_sys::init_world;
use neutronium::prelude::World;
use std::env::current_dir;

fn main() {
    let matches = App::new("Game Server")
        .version("1.0")
        .author("Bush Hammer Industries")
        .about("Runs the game server.")
        .arg(
            Arg::with_name("CONFIG_FILE")
                .help("Path to the config file")
                .default_value("game_config.toml"),
        )
        .get_matches();

    // Initialize logging
    let log = logging::init();

    logging::info!(log, ""; "working_directory" => ?current_dir().unwrap());

    let config_file_path = matches.value_of("CONFIG_FILE").unwrap();
    logging::info!(log, "reading configuration file path";
                   "context" => "main",
                   "config_file_path" => config_file_path);

    logging::info!(log, "parsing configuration");
    let config = GameConfig::load(config_file_path);
    logging::info!(log, "parsed configuration";
                   "context" => "main",
                   "server_address" => &config.server.address,
                   "server_max_clients" => config.server.max_clients,
                   "server_threads" => config.server.threads,
                   "game_fps" => config.game.fps);

    let mut world = World::new(config.game.fps, &log);

    logging::info!(log, "initializing world instance"; "context" => "main",);
    init_world(&mut world, &config, &log);
    logging::info!(log, "world instance initialized"; "context" => "main",);

    logging::info!(log, "starting game loop"; "context" => "main",);
    world.run();
}
