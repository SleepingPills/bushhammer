use serdeconv;
use slog;
use sloggers;
use sloggers::{Config, LoggerConfig};

pub fn init() -> slog::Logger {
    /*
    TODO:
    1. Create function to read in logging config from file based on executable name (or create a default
       terminal outputter).
    2. Hook up config to the authenticator
    3. Hook up config to the endpoint
    */

    let config: LoggerConfig = serdeconv::from_toml_str(
        r#"
type = "terminal"
level = "trace"
destination = "stderr""#,
    )
    .unwrap();

    config.build_logger().unwrap()
}
