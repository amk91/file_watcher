use std::{
    path::PathBuf,
    sync::{
        Arc, Mutex,
        mpsc::{self},
    },
    thread,
    time::Duration,
};

use crate::config::Config;

mod monitor_folders;
mod handle_files;
mod monitor_config;

struct MovingInfo {
    pub timeout: Duration,
    pub attempts: u8,
    pub source_folder: String,
    pub destination_folder: String,
}

pub struct App {
    config: Arc<Mutex<Config>>,
}

impl App {
    pub fn new() -> App {
        App {
            config: Arc::new(Mutex::new(Config::default().init())),
        }
    }

    pub fn run(&mut self) {
        let (tx_new_file_event, rx_new_file_event) = mpsc::channel::<PathBuf>();
        let (tx_config_updated, rx_config_updated) = mpsc::channel::<()>();

        let config = self.config.clone();
        let monitor_config_thread = thread::spawn(|| {
            App::monitor_config(config, tx_config_updated);
        });

        let config = self.config.clone();
        let monitor_folders_thread = thread::spawn(|| {
            App::monitor_folders(config, tx_new_file_event, rx_config_updated);
        });

        let config = self.config.clone();
        let handle_files_thread = thread::spawn(|| {
            App::handle_files(config, rx_new_file_event);
        });

        monitor_config_thread.join().unwrap();
        monitor_folders_thread.join().unwrap();
        handle_files_thread.join().unwrap();
    }
}
