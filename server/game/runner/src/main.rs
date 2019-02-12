use clap::{App, Arg};
use flux::logging;
use gamecore::config::GameConfig;
use gamecore::init_sys::init_world;
use neutronium::prelude::World;

fn main() {
    let matches = App::new("Game Server")
        .version("1.0")
        .author("Bush Hammer Industries")
        .about("Runs the game server.")
        .arg(
            Arg::with_name("CONFIG_FILE")
                .help("Path to the config file")
                .required(true),
        )
        .get_matches();

    // Initialize logging
    let log = logging::init();

    let config_file_path = matches.value_of("CONFIG_FILE").unwrap();
    logging::info!(log, "reading configuration file path";
                   "context" => "main",
                   "config_file_path" => config_file_path);

    logging::info!(log, "parsing configuration");
    let config = GameConfig::load(config_file_path);
    logging::info!(log, "parsed configuration"; "config" => config);

    let mut world = World::new(config.game.fps);

    logging::info!(log, "initializing world instance");
    init_world(&mut world, &log);
    logging::info!(log, "world instance initialized");

    logging::info!(log, "starting game loop");
    world.run();
}
