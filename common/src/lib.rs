use std::path::PathBuf;

use tracing::{Level, error, level_filters::LevelFilter};
use tracing_appender::non_blocking::WorkerGuard;
use tracing_subscriber::{Layer, layer::SubscriberExt, util::SubscriberInitExt};

pub mod app_paths;
pub mod config;

pub const APP_NAME: &str = "file-watcher";

pub fn init_tracing(log_dir: &PathBuf) -> Option<WorkerGuard> {
    if cfg!(debug_assertions) {
        tracing_subscriber::fmt()
            .with_max_level(Level::TRACE)
            .init();

        None
    } else {
        let file_appender = tracing_appender::rolling::daily(log_dir, "file_watcher_service.log");
        let (non_blocking, guard) = tracing_appender::non_blocking(file_appender);

        let file_layer = tracing_subscriber::fmt::layer()
            .with_writer(non_blocking)
            .with_filter(LevelFilter::INFO);

        let registry = tracing_subscriber::registry().with(file_layer);

        match tracing_journald::layer() {
            Ok(layer) => {
                registry.with(layer.with_filter(LevelFilter::TRACE)).init();
            }
            Err(err) => {
                registry.init();
                error!(?err, "Unable to register journalctl layer to tracing");
            }
        }

        Some(guard)
    }
}
