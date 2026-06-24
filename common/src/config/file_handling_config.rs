use std::{fmt::Display, time::Duration};

use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Debug, PartialEq, Clone)]
pub enum FileOperationaType {
    Move,
    Copy,
}

impl Default for FileOperationaType {
    fn default() -> Self {
        FileOperationaType::Move
    }
}

impl Display for FileOperationaType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let text = match self {
            FileOperationaType::Move => "move",
            FileOperationaType::Copy => "copy",
        };
        write!(f, "{}", text)
    }
}

#[derive(Serialize, Deserialize, Debug, Default, PartialEq, Clone)]
pub struct FolderMonitor {
    pub enabled: bool,
    pub file_operation_type: FileOperationaType,
    pub extensions: Vec<String>,
    pub source_folder: String,
    pub destination_folder: String,
}

#[derive(Serialize, Deserialize, Debug, Default, PartialEq, Clone)]
pub struct FileHandlingConfig {
    pub part_temp_file_check: bool,
    pub folder_monitors: Vec<FolderMonitor>,
    pub move_attempts: u8,
    pub check_interval: Duration,
    pub file_timeout: Duration,
}
