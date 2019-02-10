use clap::{App, Arg};
use flux::logging;
use gamecore::config::GameConfig;

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
    let logger = logging::init();

    let config_file_path = matches.value_of("CONFIG_FILE").unwrap();
    logging::debug!(logger, "reading configuration file path";
                    "context" => "main",
                    "config_file_path" => config_file_path);

    let config = GameConfig::load(config_file_path);

    /*
    TODO
    - Create config structs
    - Deserialize config structs
    - Create replicator system with endpoint inside
    - Create world instance
    - Register replicator system

    research_technology tech_genome_mapping
    research_technology tech_frontier_health
    research_technology galactic_administration
    influence 200

    if you dont want to wait three months you can just fire the event bioexpanded.1
    */
}
