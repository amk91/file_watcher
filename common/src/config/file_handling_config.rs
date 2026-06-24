use std::{fmt::Display, time::Duration};

use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Debug, PartialEq, Clone)]
pub enum FileOperationType {
    Move,
    Copy,
}

impl Default for FileOperationType {
    fn default() -> Self {
        FileOperationType::Move
    }
}

impl Display for FileOperationType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let text = match self {
            FileOperationType::Move => "move",
            FileOperationType::Copy => "copy",
        };
        write!(f, "{}", text)
    }
}

#[derive(Serialize, Deserialize, Debug, Default, PartialEq, Clone)]
pub struct FolderMonitor {
    #[serde(default = "FolderMonitor::enabled_default")]
    pub enabled: bool,
    #[serde(default = "FileOperationType::default")]
    pub file_operation_type: FileOperationType,
    pub extensions: Vec<String>,
    pub source_folder: String,
    pub destination_folder: String,
}

impl FolderMonitor {
    fn enabled_default() -> bool {
        true
    }
}

#[derive(Serialize, Deserialize, Debug, Default, PartialEq, Clone)]
pub struct FileHandlingConfig {
    #[serde(default = "FileHandlingConfig::part_temp_file_check_default")]
    pub part_temp_file_check: bool,
    pub folder_monitors: Vec<FolderMonitor>,
    #[serde(default = "FileHandlingConfig::move_attempts_default")]
    pub move_attempts: u8,
    #[serde(default = "FileHandlingConfig::check_interval_default")]
    pub check_interval: Duration,
    #[serde(default = "FileHandlingConfig::file_timeout_default")]
    pub file_timeout: Duration,
}

impl FileHandlingConfig {
    fn part_temp_file_check_default() -> bool {
        true
    }

    fn move_attempts_default() -> u8 {
        5
    }

    fn check_interval_default() -> Duration {
        Duration::from_secs(1)
    }

    fn file_timeout_default() -> Duration {
        Duration::from_secs(1)
    }
}
