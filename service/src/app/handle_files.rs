use std::{
    collections::HashMap,
    path::{Path, PathBuf},
    sync::{Arc, RwLock},
    time::{Duration, Instant},
};

use anyhow::anyhow;
use common::config::file_handling_config::{FileHandlingConfig, FileOperationaType};
use crossbeam_channel::{Receiver, Sender, select};
use tracing::{error, info, trace, warn};

use crate::{
    app::App,
    history_manager::{EventType, FileEventInfo},
};

#[derive(Debug)]
struct MovingInfo {
    pub file_operation_type: FileOperationaType,
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
        trace!(?config);
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
                    let mut files_to_handle = vec![];
                    for (filename, moving_info) in files_list.iter_mut() {
                        moving_info.timeout = moving_info.timeout.saturating_sub(check_interval);
                        if moving_info.timeout == Duration::ZERO {
                            files_to_handle.push((filename.clone(), moving_info.file_operation_type.clone()));
                            info!(
                                "Safety timeout for {} triggered, file is flagged for removal",
                                filename.display()
                            );
                        }
                    }

                    for (filename, file_operation_type) in &mut files_to_handle {
                        App::handle_file(&mut files_list, filename, file_operation_type.clone(), &tx_event);
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
        trace!("Event received for {}", filename.display());

        let filename = if part_temp_file_check
            && let Some(ext) = filename.extension()
            && ext == "part"
            && let Some(filename) = filename.file_stem()
        {
            trace!("Part file for {} detected", filename.display());
            PathBuf::from(filename)
        } else {
            filename
        };

        let extension = match filename.extension() {
            Some(extension) => match extension.to_str() {
                Some(extension) => String::from(extension),
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
                if folder_monitor.extensions.contains(&extension) {
                    info!(
                        "File with extension {extension} found: {}",
                        filename.display()
                    );

                    if files_list.contains_key(&filename) {
                        files_list.remove(&filename);
                        trace!(
                            "Possible temp file {} already registered and will be removed",
                            filename.display()
                        );
                    } else {
                        trace!("Add file {} to monitored list", filename.display());
                        files_list.insert(
                            filename,
                            MovingInfo {
                                file_operation_type: folder_monitor.file_operation_type.clone(),
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

    fn handle_file(
        files_list: &mut HashMap<PathBuf, MovingInfo>,
        filename: &mut PathBuf,
        file_operation_type: FileOperationaType,
        tx_event: &Sender<EventType>,
    ) {
        let mut remove = false;
        if let Some(moving_info) = files_list.get_mut(filename) {
            let handle_result = match file_operation_type {
                FileOperationaType::Move => App::move_file(
                    &filename,
                    &moving_info.source_folder,
                    &moving_info.destination_folder,
                ),
                FileOperationaType::Copy => App::copy_file(
                    &filename,
                    &moving_info.source_folder,
                    &moving_info.destination_folder,
                ),
            };

            match handle_result {
                Ok(_) => {
                    remove = true;
                    info!(
                        "File {} handled ({}) successfully and removed from list",
                        filename.display(),
                        moving_info.file_operation_type,
                    );

                    if let Err(err) = tx_event.send(EventType::FileMoved(FileEventInfo {
                        filepath: PathBuf::from(&moving_info.source_folder).join(&filename),
                        destination_path: PathBuf::from(&moving_info.destination_folder)
                            .join(&filename),
                    })) {
                        error!(?err, "Unable to send event to history manager");
                    }
                }
                Err(err) => {
                    if moving_info.attempts == 0 {
                        remove = true;
                        warn!(
                            ?err,
                            "Unable to handle ({}) {} from {} to {}",
                            moving_info.file_operation_type,
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

    fn check_paths_before_handling(
        filename: &PathBuf,
        source_folder: &String,
        destination_folder: &String,
    ) -> Result<(PathBuf, PathBuf), anyhow::Error> {
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

        Ok((source_filepath, destination_filepath))
    }

    fn move_file(
        filename: &PathBuf,
        source_folder: &String,
        destination_folder: &String,
    ) -> Result<(), anyhow::Error> {
        let (source_filepath, destination_filepath) =
            match App::check_paths_before_handling(filename, source_folder, destination_folder) {
                Ok(paths) => paths,
                Err(err) => return Err(err),
            };

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
                trace!("Cross device transfer not allowed, copy-remove data");
                std::fs::copy(&source_filepath, &destination_filepath).map(|_| ())?;
                std::fs::remove_file(&source_filepath)
                    .map_err(|err| anyhow!("Unable to remove file {source_filepath:#?}: {err:#?}"))
            }
            Err(err) => Err(anyhow!(
                "Unable to rename file from {} to {}, {err:#?}",
                source_filepath.display(),
                destination_filepath.display()
            )),
        }
    }

    fn copy_file(
        filename: &PathBuf,
        source_folder: &String,
        destination_folder: &String,
    ) -> Result<(), anyhow::Error> {
        let (source_filepath, destination_filepath) =
            match App::check_paths_before_handling(filename, source_folder, destination_folder) {
                Ok(paths) => paths,
                Err(err) => return Err(err),
            };

        match std::fs::copy(&source_filepath, &destination_filepath) {
            Ok(_) => {
                info!(
                    "File {} successfully copied to {}",
                    source_filepath.display(),
                    destination_filepath.display(),
                );
                Ok(())
            }
            Err(err) => Err(anyhow!(
                "Unable to copy file from {} to {}, {err}",
                source_filepath.display(),
                destination_filepath.display()
            )),
        }
    }
}
