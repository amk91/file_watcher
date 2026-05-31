use std::time::Duration;

use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Debug)]
pub struct Config {
    pub part_temp_file_check: bool,
    pub extensions: Vec<String>,
    pub move_attempts: u8,
    pub monitored_folder: String,
    pub destination_folder: String,
    pub check_interval: Duration,
    pub file_timeout: Duration,
    pub thread_sleep: Duration,
}

impl ::std::default::Default for Config {
    fn default() -> Self {
        let (monitored_folder, destination_folder) = if let Some(home_path) = std::option_env!("HOME") {
            (
                format!("{home_path}/Downloads/"),
                format!("{home_path}/file_janitor/"),
            )
        } else {
            ("".into(), "".into())
        };

        Self {
            part_temp_file_check: true,
            extensions: vec!["3mf".into(), "stl".into()],
            move_attempts: 5u8,
            monitored_folder,
            destination_folder,
            check_interval: Duration::from_millis(1000),
            file_timeout: Duration::from_millis(5000),
            thread_sleep: Duration::from_millis(100),
        }
    }
}
