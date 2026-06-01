use std::time::Duration;

use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Debug)]
pub struct Config {
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
            part_temp_file_check: true,
            folder_monitors: vec![],
            move_attempts: 5u8,
            check_interval: Duration::from_millis(1000),
            file_timeout: Duration::from_millis(5000),
            thread_sleep: Duration::from_millis(100),
        }
    }
}
