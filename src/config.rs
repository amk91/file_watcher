use std::{fs::{File, OpenOptions}, io::{ErrorKind::{self, NotFound}, Read, Write}, path::PathBuf, time::Duration};

use serde::{Deserialize, Serialize};
use tracing::{error, trace, warn};

pub const CONFIG_FILENAME: &str = "config.yml";
pub const HISTORY_FILENAME: &str = "history.json";

#[derive(Serialize, Deserialize, Debug, PartialEq, Clone)]
pub struct Config {
    pub file_handling_config: FileHandlingConfig,
    pub history_config: HistoryConfig,
}

#[derive(Serialize, Deserialize, Debug, Default, PartialEq, Clone)]
pub struct FileHandlingConfig {
    pub part_temp_file_check: bool,
    pub folder_monitors: Vec<FolderMonitor>,
    pub move_attempts: u8,
    pub check_interval: Duration,
    pub file_timeout: Duration,
}

#[derive(Serialize, Deserialize, Debug, Default, PartialEq, Clone)]
pub struct HistoryConfig {
    pub filepath: PathBuf,
    pub max_size_mb: usize,
    pub flush_interval: Duration,
}

#[derive(Serialize, Deserialize, Debug, Default, PartialEq, Clone)]
pub struct FolderMonitor {
    pub extensions: Vec<String>,
    pub source_folder: String,
    pub destination_folder: String,
}

impl ::std::default::Default for Config {
    fn default() -> Self {
        Self {
            file_handling_config: FileHandlingConfig {
                part_temp_file_check: true,
                //TODO: check for duplicate FolderMonitor
                folder_monitors: vec![],
                move_attempts: 5u8,
                check_interval: Duration::from_millis(1000),
                file_timeout: Duration::from_millis(5000),
            },
            history_config: HistoryConfig {
                filepath: "".into(),
                max_size_mb: 5,
                flush_interval: Duration::from_millis(1000),
            },
        }
    }
}

impl Config {
    pub fn init(config_path: &PathBuf, data_dir: &PathBuf) -> Self {
        let mut config = match File::open(&config_path) {
            Ok(mut file) => {
                let mut config_buffer = String::new();
                match file.read_to_string(&mut config_buffer) {
                    Ok(_) => {
                        match yaml_serde::from_str::<Config>(&config_buffer) {
                            Ok(config) => {
                                trace!(?config, "Configuration loaded from yml file");
                                config
                            },
                            Err(err) => {
                                warn!(?err, "Unable to parse configuration from file");

                                let config_path_bak = PathBuf::from(&config_path).with_extension("yaml.bak");
                                if let Err(err) = std::fs::copy(&config_path, config_path_bak) {
                                    warn!(?err, "Unable to copy config file to a bak file");
                                }

                                Config::default()
                            },
                        }
                    },
                    Err(err) => {
                        warn!(?err, "Unable to read file to string");
                        Config::default()
                    },
                }
            },
            Err(err) if err.kind() == NotFound => {
                match File::create_new(&config_path) {
                    Ok(_) => Config::default(),
                    Err(err) => {
                        let err_string = format!("Unable to generate config file at {}", config_path.display());
                        error!(?err, err_string);
                        panic!("{err_string}: {err}");
                    }
                }
            },
            Err(err) => {
                let err_string = format!("Unable to open config file at {}", config_path.display());
                error!(?err, err_string);
                panic!("{err_string}: {err}");
            },
        };

        if config.history_config.filepath.as_os_str().is_empty()
            || config.history_config.filepath.is_relative()
        {
            let history_filepath = PathBuf::from(&data_dir).join(HISTORY_FILENAME);
            warn!(
                "Invalid history config filepath: {}, path set to {}",
                config.history_config.filepath.display(),
                history_filepath.display()
            );

            if let Err(err) = std::fs::create_dir_all(&history_filepath)
                && err.kind() != ErrorKind::AlreadyExists
            {
                warn!(
                    ?err,
                    "Unable to generate directories for history filepath at {}, history will be disabled",
                    history_filepath.display()
                );
            } else {
                trace!(
                    "History filepath updated from {} to {}",
                    config.history_config.filepath.display(),
                    history_filepath.display()
                );
                config.history_config.filepath = history_filepath;
            }
        }

        match yaml_serde::to_string(&config) {
            Ok(config_string) => {
                match OpenOptions::new().write(true).open(&config_path) {
                    Ok(mut file) => if let Err(err) = file.write_all(config_string.as_bytes()) {
                        error!(?err, "Unable to write to config yaml string to file");
                    },
                    Err(err) => error!(?err, "Unable to open config file in write mode"),
                }
            }
            Err(err) => error!(?err, "Unable to parse configuration as yaml file")
        }

        config
    }
}
