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
        let config = match confy::load("file_janitor", Some("config")) {
            Ok(config) => config,
            Err(err) => panic!("Unable to load configuration: {err:#?}"),
        };

        App {
            config: Arc::new(Mutex::new(config)),
        }
    }

    pub fn run(&mut self) {
        let (sender, receiver) = mpsc::channel::<PathBuf>();

        let config = self.config.clone();
        let monitor_config_thread = thread::spawn(|| {

        });

        let config = self.config.clone();
        let monitor_folders_thread = thread::spawn(|| {
            App::monitor_folders(config, sender);
        });

        let config = self.config.clone();
        let handle_files_thread = thread::spawn(|| {
            App::handle_files(config, receiver);
        });

        monitor_config_thread.join().unwrap();
        monitor_folders_thread.join().unwrap();
        handle_files_thread.join().unwrap();
    }
}
