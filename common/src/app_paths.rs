use std::path::PathBuf;

use directories::ProjectDirs;

use crate::{APP_NAME, config::CONFIG_FILENAME};

#[derive(Debug)]
pub struct AppPaths {
    pub data_dir: PathBuf,
    pub config_dir: PathBuf,
    pub config_path: PathBuf,
}

impl AppPaths {
    pub fn new() -> Self {
        let (data_dir, config_dir) = match ProjectDirs::from("", "amk319", APP_NAME) {
            Some(proj_dirs) => (
                PathBuf::from(&proj_dirs.data_dir()),
                PathBuf::from(&proj_dirs.config_dir()),
            ),
            None => panic!("Unable to retrieve projects folders, unable to continue"),
        };

        Self {
            data_dir,
            config_dir: config_dir.clone(),
            config_path: PathBuf::from(config_dir).join(CONFIG_FILENAME),
        }
    }
}
