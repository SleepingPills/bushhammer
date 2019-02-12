use gamecore::config::GameConfig;
use serdeconv;

fn main() {
    let config = serdeconv::to_toml_string(&GameConfig::default()).expect("Failed to generate config file");

    println!("{}", config);
}
