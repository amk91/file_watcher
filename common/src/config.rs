use std::{
    fs::{self, File},
    path::{Path, PathBuf},
    time::Duration,
};

use serde::{Deserialize, Serialize};
use tracing::{error, trace, warn};

pub mod file_handling_config;
use file_handling_config::FileHandlingConfig;

pub const CONFIG_FILENAME: &str = "config.json";
pub const HISTORY_FILENAME: &str = "history.json";

#[derive(Serialize, Deserialize, Debug, Default, PartialEq, Clone)]
pub struct HistoryConfig {
    pub filepath: PathBuf,
    #[serde(default = "HistoryConfig::max_size_mb_default")]
    pub max_size_mb: usize,
    #[serde(default = "HistoryConfig::flush_interval_default")]
    pub flush_interval: Duration,
}

impl HistoryConfig {
    fn max_size_mb_default() -> usize {
        5
    }

    fn flush_interval_default() -> Duration {
        Duration::from_secs(1)
    }
}

#[derive(Serialize, Deserialize, Debug, PartialEq, Clone)]
pub struct Config {
    pub file_handling_config: FileHandlingConfig,
    pub history_config: HistoryConfig,
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
    pub fn init(config_path: &Path, config_dir: &Path, data_dir: &PathBuf) -> Self {
        fs::create_dir_all(config_dir).unwrap_or_else(|err| {
            panic!(
                "Unable to generate app config folder at {}: {err}",
                config_dir.display()
            );
        });

        let create_config_file = || {
            if let Err(err) = File::create_new(&config_path) {
                panic!(
                    "Unable to generate config file at {}: {err}",
                    config_path.display()
                );
            }
        };

        let backup_and_recreate = || {
            let backup_path = config_path.with_extension("json.bak");
            fs::rename(config_path, &backup_path).unwrap_or_else(|err| {
                panic!(
                    "Unable to rename config file at {}: {err}",
                    config_path.display()
                );
            });
            create_config_file();
        };

        let mut config = if !config_path.exists() {
            create_config_file();
            Config::default()
        } else {
            match fs::read_to_string(config_path) {
                Ok(buffer) => match serde_json::from_str::<Config>(&buffer) {
                    Ok(parsed_config) => parsed_config,
                    Err(err) => {
                        warn!(
                            "Unable to parse config file at {}: {err}",
                            config_path.display()
                        );
                        backup_and_recreate();
                        Config::default()
                    }
                },
                Err(err) => {
                    warn!(
                        "Unable to read config file at {}: {err}",
                        config_path.display()
                    );
                    backup_and_recreate();
                    Config::default()
                }
            }
        };

        if config.history_config.filepath.as_os_str().is_empty()
            || config.history_config.filepath.is_relative()
        {
            let history_filepath = data_dir.join(HISTORY_FILENAME);
            warn!(
                "Invalid history config filepath: {}, path set to {}",
                config.history_config.filepath.display(),
                history_filepath.display()
            );

            if let Some(parent) = history_filepath.parent() {
                if let Err(err) = fs::create_dir_all(parent) {
                    warn!(
                        ?err,
                        "Unable to generate directories from history filepath at {}, history will be disabled",
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
        }

        config.check_for_duplicate_monitors();

        match serde_json::to_string_pretty(&config) {
            Ok(config_string) => {
                if let Err(err) = fs::write(config_path, config_string) {
                    error!(?err, "Unable to write config to file");
                }
            }
            Err(err) => error!(?err, "Unable to parse configuration as json file"),
        }

        config
    }

    pub fn check_for_duplicate_monitors(&mut self) {
        let mut duplicates = Vec::new();
        let len = self.file_handling_config.folder_monitors.len();
        for i in 0..len {
            for j in (i + 1)..len {
                let a = &self.file_handling_config.folder_monitors[i];
                let b = &self.file_handling_config.folder_monitors[j];

                if a.enabled && b.enabled {
                    let same_source_and_dest = a.source_folder == b.source_folder
                        && a.destination_folder == b.destination_folder;
                    let same_ext_and_source =
                        a.extensions == b.extensions && a.source_folder == b.source_folder;

                    if same_source_and_dest || same_ext_and_source {
                        duplicates.push(j);
                        warn!(
                            ?b,
                            "Duplicate folder monitor is disabled because {}",
                            if same_source_and_dest {
                                "it has same source and same destination"
                            } else {
                                "it has same extensions list and same source"
                            }
                        );
                    }
                }
            }
        }

        for i in duplicates {
            self.file_handling_config.folder_monitors[i].enabled = false;
        }
    }
}
