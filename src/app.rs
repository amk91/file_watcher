use std::{
    path::PathBuf,
    sync::{
        Arc, RwLock,
        mpsc::{self},
    },
    thread,
};

use tracing::trace;

use directories::ProjectDirs;

use crate::config::{CONFIG_FILENAME, Config, FileHandlingConfig, HistoryConfig};

mod handle_files;
mod history_manager;
mod monitor_config;
mod monitor_folders;

pub const APP_NAME: &str = env!("CARGO_PKG_NAME");

#[derive(Debug)]
struct AppPaths {
    data_dir: PathBuf,
    _config_dir: PathBuf,
    config_path: PathBuf,
}

impl AppPaths {
    pub fn new(data_dir: PathBuf, config_dir: PathBuf) -> Self {
        Self {
            data_dir,
            _config_dir: config_dir.clone(),
            config_path: PathBuf::from(config_dir).join(CONFIG_FILENAME),
        }
    }
}

#[derive(Debug)]
pub struct App {
    file_handling_config: Arc<RwLock<FileHandlingConfig>>,
    _history_config: Arc<RwLock<HistoryConfig>>,
    app_paths: AppPaths,
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

        let app_paths = AppPaths::new(data_dir, config_dir);
        let config = Config::init(&app_paths.config_path, &app_paths.data_dir);

        App {
            file_handling_config: Arc::new(RwLock::new(config.file_handling_config)),
            _history_config: Arc::new(RwLock::new(config.history_config)),
            app_paths,
        }
    }

    pub fn run(&mut self) {
        let (tx_new_file_event, rx_new_file_event) = mpsc::channel::<PathBuf>();
        let (tx_config_updated, rx_config_updated) = mpsc::channel::<()>();

        trace!("Spawning configuration monitor thread");
        let file_handling_config = self.file_handling_config.clone();
        let config_path = self.app_paths.config_path.clone();
        let monitor_config_thread = thread::spawn(|| {
            App::monitor_config(config_path, tx_config_updated, file_handling_config)
        });

        trace!("Spawning folders monitor thread");
        let config = self.file_handling_config.clone();
        let monitor_folders_thread = thread::spawn(|| {
            App::monitor_folders(config, tx_new_file_event, rx_config_updated);
        });

        trace!("Spawning file handling thread");
        let config = self.file_handling_config.clone();
        let handle_files_thread = thread::spawn(|| {
            App::handle_files(config, rx_new_file_event);
        });

        monitor_config_thread.join().unwrap();
        monitor_folders_thread.join().unwrap();
        handle_files_thread.join().unwrap();
    }
}
