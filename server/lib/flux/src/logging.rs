use serdeconv;
use slog;
use sloggers;
use sloggers::{Config, LoggerConfig};
use std::env::current_exe;

pub use slog::{trace, debug, info, warn, error, crit};

const LOG_CONFIG: &str = r#"
type = "terminal"
level = "trace"
destination = "stderr""#;

pub fn init() -> slog::Logger {
    let mut path = current_exe().expect("Logging: failed to retrieve executable");

    if !path.is_file() {
        panic!("Logging: invalid executable path")
    }

    path.set_extension(".log.toml");

    if path.exists() {
        let config: LoggerConfig = serdeconv::from_json_file(&path).expect("");
        let logger = config.build_logger().expect("Logging: invalid configuration");
        slog::info!(logger, "log config file: {log_config_file}", log_config_file = path.to_str().unwrap());
        logger
    } else {
        let config: LoggerConfig = serdeconv::from_toml_str(LOG_CONFIG).unwrap();
        let logger = config.build_logger().expect("Logging: invalid configuration");
        slog::warn!(logger, "log config file not found, using defaults";
                    "type" => "terminal",
                    "level" => "trace",
                    "destination" => "stderr");
        logger
    }
}
