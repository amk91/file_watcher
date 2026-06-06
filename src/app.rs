use std::{
    path::PathBuf,
    sync::{
        Arc, Mutex,
        mpsc::{self},
    },
    thread,
    time::Duration,
};

use tracing::{info, trace};

use directories::ProjectDirs;

use crate::config::{Config, CONFIG_NAME};

mod monitor_folders;
mod handle_files;
mod monitor_config;

#[derive(Debug)]
struct MovingInfo {
    pub timeout: Duration,
    pub attempts: u8,
    pub source_folder: String,
    pub destination_folder: String,
}

pub const APP_NAME: &str = env!("CARGO_PKG_NAME");

#[derive(Debug)]
struct AppPaths {
    _data_dir: PathBuf,
    _config_dir: PathBuf,
    config_path: PathBuf,
}

impl AppPaths {
    pub fn new(data_dir: PathBuf, config_dir: PathBuf) -> Self {
        Self {
            _data_dir: data_dir,
            _config_dir: config_dir.clone(),
            config_path: PathBuf::from(config_dir).join(CONFIG_NAME)
        }
    }
}

#[derive(Debug)]
pub struct App {
    config: Arc<Mutex<Config>>,
    app_paths: AppPaths,
}

impl App {
    pub fn new() -> App {
        let (data_dir, config_dir) = match ProjectDirs::from("", "amk319", APP_NAME) {
            Some(proj_dirs) => (PathBuf::from(&proj_dirs.data_dir()), PathBuf::from(&proj_dirs.config_dir())),
            None => panic!("Unable to retrieve projects folders, unable to continue"),
        };

        App {
            config: Arc::new(Mutex::new(Config::default().init(config_dir.clone()))),
            app_paths: AppPaths::new(data_dir, config_dir),
        }
    }

    pub fn run(&mut self) {
        let (tx_new_file_event, rx_new_file_event) = mpsc::channel::<PathBuf>();
        let (tx_config_updated, rx_config_updated) = mpsc::channel::<()>();

        trace!("Spawning configuration monitor thread");
        let config = self.config.clone();
        let config_path = self.app_paths.config_path.clone();
        let monitor_config_thread = thread::spawn(|| {
            App::monitor_config(config, config_path, tx_config_updated);
        });

        trace!("Spawning folders monitor thread");
        let config = self.config.clone();
        let monitor_folders_thread = thread::spawn(|| {
            App::monitor_folders(config, tx_new_file_event, rx_config_updated);
        });

        trace!("Spawning file handling thread");
        let config = self.config.clone();
        let handle_files_thread = thread::spawn(|| {
            App::handle_files(config, rx_new_file_event);
        });

        monitor_config_thread.join().unwrap();
        monitor_folders_thread.join().unwrap();
        handle_files_thread.join().unwrap();
    }
}
