use std::{path::PathBuf, time::Duration};

use serde::{Deserialize, Serialize};
use tracing::{error, warn};

pub const CONFIG_FILENAME: &str = "config.toml";
pub const HISTORY_FILENAME: &str = "history.json";

#[derive(Serialize, Deserialize, Debug)]
pub struct Config {
    pub file_handling_config: FileHandlingConfig,
    pub history_config: HistoryConfig,
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
pub struct HistoryConfig {
    pub filepath: PathBuf,
    pub max_size_mb: usize,
    pub flush_interval: Duration,
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
                folder_monitors: vec![],
                move_attempts: 5u8,
                check_interval: Duration::from_millis(1000),
                file_timeout: Duration::from_millis(5000),
                thread_sleep: Duration::from_millis(100),
            },
            history_config: HistoryConfig {
                filepath: "".into(),
                max_size_mb: 5,
                flush_interval: Duration::from_millis(1000),
                thread_sleep: Duration::from_millis(100),
            },
        }
    }
}

impl Config {
    pub fn init(config_path: &PathBuf, data_dir: &PathBuf) -> Self {
        match confy::load_path::<Config>(config_path) {
            Ok(mut config) => {
                if config.history_config.filepath.as_os_str().is_empty()
                    || config.history_config.filepath.is_relative()
                {
                    warn!("Invalid history config filepath: {}", config.history_config.filepath.display());
                    config.history_config.filepath = PathBuf::from(&data_dir).join(HISTORY_FILENAME);
                }

                config
            }
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
