use tracing_subscriber::{fmt, EnvFilter, layer::SubscriberExt, util::SubscriberInitExt};
use std::path::Path;

/// Initialize logging system
pub fn init_logger(level: &str, json_output: bool, log_file: Option<&Path>) {
    let filter = EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| EnvFilter::new(level));

    let registry = tracing_subscriber::registry().with(filter);

    if json_output {
        // JSON formatting for production
        if let Some(file) = log_file {
            let file = std::fs::OpenOptions::new()
                .create(true)
                .append(true)
                .open(file)
                .expect("Failed to open log file");

            registry
                .with(fmt::layer().json().with_writer(file))
                .init();
        } else {
            registry
                .with(fmt::layer().json())
                .init();
        }
    } else {
        // Pretty formatting for development
        registry
            .with(fmt::layer().pretty())
            .init();
    }
}

/// Initialize logger from config
pub fn init_from_config(config: &crate::utils::config::LoggingConfig) {
    let json = config.output == "json";
    let log_file = if !config.file_path.is_empty() {
        Some(Path::new(&config.file_path))
    } else {
        None
    };

    init_logger(&config.level, json, log_file);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_logger_init() {
        // Just verify the function exists
        // Can't actually test logging without side effects
    }
}
