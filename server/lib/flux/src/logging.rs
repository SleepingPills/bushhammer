use serdeconv;
use slog;
use sloggers;

pub fn init() {
    /*
    TODO:
    1. Create function to read in logging config from file based on executable name (or create a default
       terminal outputter).
    2. Hook up config to the authenticator
    3. Hook up config to the endpoint
    */
    use sloggers::{Config, LoggerConfig};

    let config: LoggerConfig = serdeconv::from_toml_str(
        r#"
type = "terminal"
level = "debug"
destination = "stderr"
"#,
    )
    .unwrap();

    let logger = config.build_logger().unwrap();
}
