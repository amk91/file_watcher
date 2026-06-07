use std::{
    collections::HashMap,
    path::{Path, PathBuf},
    sync::{Arc, RwLock},
    time::{Duration, Instant},
};

use anyhow::anyhow;
use crossbeam_channel::{Receiver, Sender, select};
use tracing::{error, info, warn};

use crate::{
    app::{App, history_manager::EventType},
    config::FileHandlingConfig,
};

#[derive(Debug)]
struct MovingInfo {
    pub timeout: Duration,
    pub attempts: u8,
    pub source_folder: String,
    pub destination_folder: String,
}

impl App {
    pub fn handle_files(
        config: Arc<RwLock<FileHandlingConfig>>,
        receiver: Receiver<PathBuf>,
        tx_event: Sender<EventType>,
    ) {
        let mut files_list: HashMap<PathBuf, MovingInfo> = HashMap::new();

        let config_locked = config.read().expect("Unable to acquire lock");
        let check_interval = config_locked.check_interval;
        let part_temp_file_check = config_locked.part_temp_file_check;
        drop(config_locked);

        let mut next_check = Instant::now() + check_interval;
        loop {
            let time_to_next_check = next_check.saturating_duration_since(Instant::now());
            select! {
                recv(receiver) -> filename => {
                    match filename {
                        Ok(filename) => App::update_files_list(filename, part_temp_file_check, &config, &mut files_list),
                        Err(err) => error!(?err, "Unable to retrieve event")
                    }
                }

                recv(crossbeam_channel::after(time_to_next_check)) -> _ => {
                    let mut files_to_move = vec![];
                    for (filename, moving_info) in files_list.iter_mut() {
                        moving_info.timeout = moving_info.timeout.saturating_sub(check_interval);
                        if moving_info.timeout == Duration::ZERO {
                            files_to_move.push(filename.clone());
                            info!(
                                "Safety timeout for {} triggered, file is flagged for removal",
                                filename.display()
                            );
                        }
                    }

                    for filename in &mut files_to_move {
                        App::handle_file(&mut files_list, filename);
                    }

                    next_check = Instant::now() + check_interval;
                }
            }
        }
    }

    fn update_files_list(
        filename: PathBuf,
        part_temp_file_check: bool,
        config: &Arc<RwLock<FileHandlingConfig>>,
        files_list: &mut HashMap<PathBuf, MovingInfo>,
    ) {
        info!("event received for {}", filename.display());

        let filename = if part_temp_file_check
            && let Some(ext) = filename.extension()
            && ext == "part"
            && let Some(filename) = filename.file_stem()
        {
            info!("Part file for {} detected", filename.display());
            PathBuf::from(filename)
        } else {
            filename
        };

        let extension = match filename.extension() {
            Some(extension) => match extension.to_str() {
                Some(extension) => extension,
                None => {
                    warn!("Unable to retrieve extension from OsStr");
                    return;
                }
            },
            None => {
                warn!("Unable to retrieve extension from PathBuf");
                return;
            }
        };

        if let Ok(config) = config.read() {
            for folder_monitor in &config.folder_monitors {
                if extension == folder_monitor.extension {
                    info!(
                        "File with extension {extension} found: {}",
                        filename.display()
                    );

                    if files_list.contains_key(&filename) {
                        files_list.remove(&filename);
                        info!(
                            "Possible temp file {} already registered and will be removed",
                            filename.display()
                        );
                    } else {
                        info!("Add file {} to monitored list", filename.display());
                        files_list.insert(
                            filename,
                            MovingInfo {
                                timeout: config.file_timeout,
                                attempts: config.move_attempts,
                                source_folder: folder_monitor.source_folder.clone(),
                                destination_folder: folder_monitor.destination_folder.clone(),
                            },
                        );
                    }

                    break;
                }
            }
        }
    }

    fn handle_file(files_list: &mut HashMap<PathBuf, MovingInfo>, filename: &mut PathBuf) {
        let mut remove = false;
        if let Some(moving_info) = files_list.get_mut(filename) {
            match App::move_file(
                &filename,
                &moving_info.source_folder,
                &moving_info.destination_folder,
            ) {
                Ok(_) => {
                    remove = true;
                    info!(
                        "File {} moved successfully and removed from list",
                        filename.display()
                    );
                }
                Err(_) => {
                    if moving_info.attempts == 0 {
                        remove = true;
                        warn!(
                            "Unable to move {} from {} to {}",
                            filename.display(),
                            moving_info.source_folder,
                            moving_info.destination_folder
                        );
                    }

                    moving_info.attempts = moving_info.attempts.saturating_sub(1);
                }
            }
        }

        if remove {
            files_list.remove(filename);
        }
    }

    fn move_file(
        filename: &PathBuf,
        source_folder: &String,
        destination_folder: &String,
    ) -> Result<(), anyhow::Error> {
        let destination_filepath = Path::new(&destination_folder).join(filename);
        if std::fs::exists(&destination_filepath)? {
            return Err(anyhow!(
                "Destination file {destination_filepath:#?} already exists"
            ));
        }

        let source_filepath = Path::new(&source_folder).join(filename);
        if !std::fs::exists(&source_filepath)? {
            return Err(anyhow!("Source file {source_filepath:#?} not found"));
        }

        match std::fs::rename(&source_filepath, &destination_filepath) {
            Ok(_) => {
                info!(
                    "File {} successfully moved from {} to {}",
                    filename.display(),
                    source_folder,
                    destination_folder,
                );
                Ok(())
            }
            Err(err) if err.kind() == std::io::ErrorKind::CrossesDevices => {
                info!("Cross device transfer not allowed, copy-remove data");
                std::fs::copy(&source_filepath, &destination_filepath).map(|_| ())?;
                std::fs::remove_file(&source_filepath).map_err(|err| {
                    anyhow!("Unable to removing file {source_filepath:#?}: {err:#?}")
                })
            }
            Err(err) => Err(anyhow!(
                "Unable to rename file from {source_filepath:#?} to {destination_filepath:#?}, {err:#?}"
            )),
        }
    }
}
