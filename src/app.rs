use std::{
    collections::HashMap,
    path::{Path, PathBuf},
    sync::{
        Arc, Mutex,
        mpsc::{self, Receiver, Sender},
    },
    thread,
    time::{Duration, SystemTime},
};

use anyhow::anyhow;
use log::{error, info, warn};
use notify::{
    Event, EventKind, INotifyWatcher, RecursiveMode, Result as NotifyResult, Watcher,
    event::CreateKind,
};

use crate::config::Config;

struct MovingInfo {
    pub timeout: Duration,
    pub attempts: u8,
    pub source_folder: String,
    pub destination_folder: String,
}

pub struct App {
    config: Arc<Mutex<Config>>,
}

impl App {
    pub fn new() -> App {
        let config = match confy::load("file_janitor", Some("config")) {
            Ok(config) => config,
            Err(err) => panic!("Unable to load configuration: {err:#?}"),
        };

        App {
            config: Arc::new(Mutex::new(config)),
        }
    }

    pub fn run(&mut self) {
        let (sender, receiver) = mpsc::channel::<PathBuf>();

        let config = self.config.clone();
        let monitor_folders_thread = thread::spawn(|| {
            App::monitor_folders(config, sender);
        });

        let config = self.config.clone();
        let handle_files_thread = thread::spawn(|| {
            App::handle_files(config, receiver);
        });

        monitor_folders_thread.join().unwrap();
        handle_files_thread.join().unwrap();
    }

    fn setup_watcher(sender: Sender<PathBuf>) -> NotifyResult<INotifyWatcher> {
        notify::recommended_watcher(move |event: NotifyResult<Event>| {
            if let Ok(event) = event {
                if event.kind == EventKind::Create(CreateKind::File) && event.paths.len() > 0 {
                    let path = event.paths[0].clone();
                    if let Some(filename) = path.file_name() {
                        if let Err(err) = sender.send(filename.into()) {
                            error!(
                                "Error sending event for file {}: {err:#?}",
                                filename.display()
                            );
                        }
                    }
                }
            }
        })
    }

    fn setup_folders(
        source_folder: &String,
        destination_folder: &String,
    ) -> Result<(), anyhow::Error> {
        // Do not add a watcher for a source folder that does not exist
        if let Ok(false) | Err(_) = std::fs::exists(source_folder) {
            return Err(anyhow!(format!(
                "Source folder {source_folder} does not exist"
            )));
        }

        // Create destination folder if it does not exist
        if let Ok(false) | Err(_) = std::fs::exists(destination_folder) {
            if let Err(err) = std::fs::create_dir_all(destination_folder) {
                return Err(anyhow!(format!(
                    "Unable to create folder {destination_folder}: {err:#?}"
                )));
            }
        }

        Ok(())
    }

    fn monitor_folders(config: Arc<Mutex<Config>>, sender: Sender<PathBuf>) {
        let (tx, rx) = mpsc::channel::<PathBuf>();
        let mut watcher = match App::setup_watcher(tx) {
            Ok(watcher) => watcher,
            Err(err) => panic!("Unable to generate watcher: {err:#?}"),
        };

        if let Ok(config) = config.lock() {
            for folder_monitor in config.folder_monitors.iter().by_ref() {
                if let Err(err) = App::setup_folders(
                    &folder_monitor.source_folder,
                    &folder_monitor.destination_folder,
                ) {
                    warn!("Unable to set up folder's monitor: {err:#?}");
                    continue;
                }

                if let Err(err) = watcher.watch(
                    Path::new(&folder_monitor.source_folder),
                    RecursiveMode::NonRecursive,
                ) {
                    error!(
                        "Unable to watch folder {}: {err:#?}",
                        folder_monitor.source_folder,
                    );
                }
            }
        }

        for filename in rx {
            if let Err(err) = sender.send(filename) {
                error!("Unable to send filename over: {err:#?}");
            }
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

    fn update_files_list(
        filename: PathBuf,
        part_temp_file_check: bool,
        config: &Arc<Mutex<Config>>,
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

        if let Ok(config) = config.lock() {
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

    fn handle_files(config: Arc<Mutex<Config>>, receiver: Receiver<PathBuf>) {
        let mut files_list: HashMap<PathBuf, MovingInfo> = HashMap::new();

        let config_locked = config.lock().expect("Unable to acquire lock");
        let check_interval = config_locked.check_interval;
        let part_temp_file_check = config_locked.part_temp_file_check;
        let thread_sleep = config_locked.thread_sleep;
        drop(config_locked);

        let mut timeout = check_interval;
        let mut process_time = SystemTime::now();
        loop {
            if let Ok(filename) = receiver.try_recv() {
                App::update_files_list(filename, part_temp_file_check, &config, &mut files_list);
            }

            timeout = timeout.saturating_sub(
                thread_sleep.saturating_sub(process_time.elapsed().unwrap_or(Duration::ZERO)),
            );

            if timeout == Duration::ZERO {
                timeout = check_interval;

                let mut files_to_move = vec![];
                for (filename, moving_info) in files_list.iter_mut() {
                    moving_info.timeout = moving_info.timeout.saturating_sub(check_interval);
                    if moving_info.timeout == Duration::ZERO {
                        files_to_move.push(filename.clone());
                        info!("Safety timeout for {} triggered, file is flagged for removal", filename.display());
                    }
                }

                for filename in &mut files_to_move {
                    let mut remove = false;
                    if let Some(moving_info) = files_list.get_mut(filename) {
                        match App::move_file(&filename, &moving_info.source_folder, &moving_info.destination_folder) {
                            Ok(_) => {
                                remove = true;
                                info!("File {} moved successfully and removed from list", filename.display());
                            },
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
            }

            thread::sleep(thread_sleep);
            process_time = SystemTime::now();
        }
    }
}
