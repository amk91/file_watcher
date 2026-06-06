use std::{path::PathBuf, time::Duration};

use serde::{Deserialize, Serialize};
use tracing::error;

pub const CONFIG_FILENAME: &str = "config.toml";

#[derive(Serialize, Deserialize, Debug)]
pub struct Config {
    pub file_handling_config: FileHandlingConfig,
}

#[derive(Serialize, Deserialize, Debug, Default)]
pub struct FileHandlingConfig {
    pub part_temp_file_check: bool,
    pub folder_monitors: Vec<FolderMonitor>,
    pub move_attempts: u8,
    pub check_interval: Duration,
    pub file_timeout: Duration,
    pub thread_sleep: Duration,
}

#[derive(Serialize, Deserialize, Debug, Default)]
pub struct FolderMonitor {
    pub extension: String,
    pub source_folder: String,
    pub destination_folder: String,
}

impl ::std::default::Default for Config {
    fn default() -> Self {
        Self {
            file_handling_config: FileHandlingConfig {
                part_temp_file_check: true,
                folder_monitors: vec![FolderMonitor {
                    extension: "3mf".into(),
                    source_folder: "/home/amk319/Downloads/".into(),
                    destination_folder: "home/amk319/Documents/3mf-files/".into()
                },
                FolderMonitor {
                    extension: "stl".into(),
                    source_folder: "/home/amk319/Downloads/".into(),
                    destination_folder: "home/amk319/Documents/3mf-files/".into()
                }],
                move_attempts: 5u8,
                check_interval: Duration::from_millis(1000),
                file_timeout: Duration::from_millis(5000),
                thread_sleep: Duration::from_millis(100),
            },
        }
    }
}

impl Config {
    pub fn init(config_path: &PathBuf) -> Self {
        match confy::load_path::<Config>(config_path) {
            Ok(config) => config,
            Err(err) => {
                error!(
                    ?err,
                    "Unable to load configuration at {config_path:?}, using default config"
                );
                Config::default()
            }
        }
    }
}
