use clap::Clap;
use lazy_static::lazy_static;
use slog::{self, Drain};
use std::sync::Mutex;

pub use slog::{debug, error, info, trace, warn};

lazy_static! {
    static ref LOGGER: Mutex<Option<slog::Logger>> = Mutex::new(None);
}

#[derive(Debug, Clap)]
pub struct Config {
    /// Log level.
    #[clap(long = "log.level", default_value = "info")]
    pub level: String,
    /// Log format.
    #[clap(long = "log.format", default_value = "pretty")]
    pub format: String,
}

pub type Logger = slog::Logger;

/// Start initializes the logger.
pub fn start(cfg: Config) {
    let format_drain = match cfg.format.as_str() {
        "json" => {
            let drain = slog_json::Json::default(std::io::stderr()).fuse();
            slog_async::Async::default(drain)
        }
        _ => {
            let decorator = slog_term::TermDecorator::new().build();
            let drain = slog_term::FullFormat::new(decorator).build().fuse();
            slog_async::Async::default(drain)
        }
    };
    let level_drain = {
        let level = match cfg.level.as_str() {
            "trace" => slog::Level::Trace,
            "debug" => slog::Level::Debug,
            "info" => slog::Level::Info,
            "warning" => slog::Level::Warning,
            "error" => slog::Level::Error,
            _ => slog::Level::Info,
        };
        slog::LevelFilter::new(format_drain, level).fuse()
    };
    LOGGER
        .lock()
        .unwrap()
        .get_or_insert(slog::Logger::root(level_drain, slog::o!()));
}

/// `start` must be called before `get_logger`.
pub fn get_logger(module: &'static str) -> slog::Logger {
    LOGGER
        .lock()
        .unwrap()
        .as_ref()
        .unwrap()
        .new(slog::o!("module" => module))
}
