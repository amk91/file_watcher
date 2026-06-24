use std::{
    path::PathBuf,
    sync::{Arc, RwLock},
    thread,
};

use common::{
    app_paths::AppPaths,
    config::{Config, HistoryConfig, file_handling_config::FileHandlingConfig},
};

use crossbeam_channel::unbounded;
use tracing::{error, trace};
use tracing_appender::non_blocking::WorkerGuard;

use crate::history_manager::HistoryManager;

mod handle_files;
mod monitor_config;
mod monitor_folders;

#[derive(Debug)]
pub struct App {
    file_handling_config: Arc<RwLock<FileHandlingConfig>>,
    history_config: Arc<RwLock<HistoryConfig>>,
    app_paths: AppPaths,
    _tracing_guard: Option<WorkerGuard>,
}

impl App {
    pub fn new() -> App {
        let app_paths = AppPaths::new();

        let tracing_guard = common::init_tracing(&app_paths.data_dir.join("log"));

        let config = Config::init(
            &app_paths.config_path,
            &app_paths.config_dir,
            &app_paths.data_dir,
        );

        App {
            file_handling_config: Arc::new(RwLock::new(config.file_handling_config)),
            history_config: Arc::new(RwLock::new(config.history_config)),
            app_paths,
            _tracing_guard: tracing_guard,
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
