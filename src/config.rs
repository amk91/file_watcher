use std::{path::PathBuf, time::Duration};

use serde::{Deserialize, Serialize};
use tracing::error;

pub const CONFIG_NAME: &str = "config.toml";

#[derive(Serialize, Deserialize, Debug, Default)]
pub struct FolderMonitor {
    pub extension: String,
    pub source_folder: String,
    pub destination_folder: String,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct Config {
    pub part_temp_file_check: bool,
    pub folder_monitors: Vec<FolderMonitor>,
    pub move_attempts: u8,
    pub check_interval: Duration,
    pub file_timeout: Duration,
    pub thread_sleep: Duration,
}

impl ::std::default::Default for Config {
    fn default() -> Self {
        Self {
            part_temp_file_check: true,
            folder_monitors: vec![],
            move_attempts: 5u8,
            check_interval: Duration::from_millis(1000),
            file_timeout: Duration::from_millis(5000),
            thread_sleep: Duration::from_millis(100),
        }
    }
}

impl Config {
    pub fn init(mut self, config_path: PathBuf) -> Self {
        match confy::load_path::<Config>(config_path) {
            Ok(new_config) => self = new_config,
            Err(err) => error!(
                ?err,
                "Unable to load configuration, backtrack to use previous or default config"
            )
        }

        self
    }
}
