use std::{
    path::PathBuf,
    sync::{Arc, RwLock},
    thread,
};

use crossbeam_channel::unbounded;
use directories::ProjectDirs;
use tracing::{Level, error, level_filters::LevelFilter, trace};
use tracing_appender::non_blocking::WorkerGuard;
use tracing_subscriber::{layer::SubscriberExt, prelude::*, util::SubscriberInitExt};

use crate::config::{CONFIG_FILENAME, Config, FileHandlingConfig, HistoryConfig};

mod handle_files;
mod history_manager;
mod monitor_config;
mod monitor_folders;

use history_manager::HistoryManager;

pub const APP_NAME: &str = env!("CARGO_PKG_NAME");

#[derive(Debug)]
struct AppPaths {
    data_dir: PathBuf,
    config_path: PathBuf,
}

impl AppPaths {
    pub fn new(data_dir: PathBuf, config_dir: PathBuf) -> Self {
        Self {
            data_dir,
            config_path: PathBuf::from(config_dir).join(CONFIG_FILENAME),
        }
    }
}

#[derive(Debug)]
pub struct App {
    file_handling_config: Arc<RwLock<FileHandlingConfig>>,
    history_config: Arc<RwLock<HistoryConfig>>,
    app_paths: AppPaths,
    _tracing_guard: Option<WorkerGuard>,
}

impl App {
    pub fn new() -> App {
        let (data_dir, config_dir) = match ProjectDirs::from("", "amk319", APP_NAME) {
            Some(proj_dirs) => (
                PathBuf::from(&proj_dirs.data_dir()),
                PathBuf::from(&proj_dirs.config_dir()),
            ),
            None => panic!("Unable to retrieve projects folders, unable to continue"),
        };

        let guard = App::init_tracing(&data_dir.join("log"));

        let app_paths = AppPaths::new(data_dir, config_dir);
        let config = Config::init(&app_paths.config_path, &app_paths.data_dir);

        App {
            file_handling_config: Arc::new(RwLock::new(config.file_handling_config)),
            history_config: Arc::new(RwLock::new(config.history_config)),
            app_paths,
            _tracing_guard: guard,
        }
    }

    fn init_tracing(log_dir: &PathBuf) -> Option<WorkerGuard> {
        if cfg!(debug_assertions) {
            tracing_subscriber::fmt()
                .with_max_level(Level::TRACE)
                .init();

            None
        } else {
            let file_appender = tracing_appender::rolling::daily(log_dir, "file_watcher.log");
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

    pub fn run(&mut self) {
        let (tx_new_file_event, rx_new_file_event) = unbounded::<PathBuf>();
        let (tx_file_handling_config_updated, rx_file_handling_config_updated) = unbounded::<()>();
        let (tx_history_config_updated, rx_history_config_updated) = unbounded::<()>();
        let (tx_event, rx_event) = unbounded();

        trace!("Spawning history manager thread");
        let history_config = self.history_config.clone();
        let history_thread = thread::spawn(|| {
            let history_manager = HistoryManager::default().init(history_config);
            history_manager
                .run(rx_event, rx_history_config_updated)
                .unwrap();
        });

        trace!("Spawning configuration monitor thread");
        let file_handling_config = self.file_handling_config.clone();
        let history_config = self.history_config.clone();
        let config_path = self.app_paths.config_path.clone();
        let tx_event_monitor_config = tx_event.clone();
        let monitor_config_thread = thread::spawn(|| {
            App::monitor_config(
                config_path,
                tx_file_handling_config_updated,
                tx_history_config_updated,
                file_handling_config,
                history_config,
                tx_event_monitor_config,
            );
        });

        trace!("Spawning folders monitor thread");
        let file_handling_config = self.file_handling_config.clone();
        let tx_event_monitor_folders = tx_event.clone();
        let monitor_folders_thread = thread::spawn(|| {
            App::monitor_folders(
                file_handling_config,
                tx_new_file_event,
                rx_file_handling_config_updated,
                tx_event_monitor_folders,
            );
        });

        trace!("Spawning file handling thread");
        let file_handling_config = self.file_handling_config.clone();
        let tx_event_file_handling = tx_event.clone();
        let handle_files_thread = thread::spawn(|| {
            App::handle_files(
                file_handling_config,
                rx_new_file_event,
                tx_event_file_handling,
            );
        });

        let mut error_on_shutdown = false;
        if let Err(err) = handle_files_thread.join() {
            error!(?err, "Handling files thread panicked during shutdown");
            error_on_shutdown = true;
        }

        if let Err(err) = monitor_folders_thread.join() {
            error!(?err, "Handling files thread panicked during shutdown");
            error_on_shutdown = true;
        }

        if let Err(err) = monitor_config_thread.join() {
            error!(?err, "Handling files thread panicked during shutdown");
            error_on_shutdown = true;
        }

        if let Err(err) = history_thread.join() {
            error!(?err, "Handling files thread panicked during shutdown");
            error_on_shutdown = true;
        }

        if error_on_shutdown {
            error!("Threads panicked");
            std::process::exit(1);
        }
    }
}
