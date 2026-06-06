use std::{
    path::{Path, PathBuf},
    sync::{
        Arc, RwLock, mpsc::{self, Receiver, Sender}
    },
    thread,
    time::Duration,
};

use notify::{Event, EventKind, INotifyWatcher, RecursiveMode, Watcher, event::CreateKind};
use tracing::{error, warn};

use crate::{app::App, config::FileHandlingConfig};

impl App {
    #[tracing::instrument]
    fn setup_watcher(sender: Sender<PathBuf>) -> notify::Result<INotifyWatcher> {
        notify::recommended_watcher(move |event: notify::Result<Event>| {
            if let Ok(event) = event {
                if event.kind == EventKind::Create(CreateKind::File) && event.paths.len() > 0 {
                    let path = event.paths[0].clone();
                    if let Some(filename) = path.file_name() {
                        if let Err(err) = sender.send(filename.into()) {
                            error!(
                                ?err,
                                "Error sending event for file {}",
                                filename.display()
                            );
                        }
                    }
                }
            }
        })
    }

    fn setup_folders(config: &Arc<RwLock<FileHandlingConfig>>, watcher: &mut INotifyWatcher, watched_folders: &mut Vec<PathBuf>) {
        if let Ok(config) = config.read() {
            for folder_monitor in config.folder_monitors.iter().by_ref() {
                // Do not add a watcher for a source folder that does not exist
                if let Ok(false) | Err(_) = std::fs::exists(&folder_monitor.source_folder) {
                    warn!(
                        "Source folder {} does not exist",
                        folder_monitor.source_folder
                    );
                    continue;
                }

                // Create destination folder if it does not exist
                if let Ok(false) | Err(_) = std::fs::exists(&folder_monitor.destination_folder) {
                    if let Err(err) = std::fs::create_dir_all(&folder_monitor.destination_folder) {
                        warn!(
                            "Unable to create folder {}: {err:?}",
                            folder_monitor.destination_folder
                        );
                        continue;
                    }
                }

                if let Err(err) = watcher.watch(
                    Path::new(&folder_monitor.source_folder),
                    RecursiveMode::NonRecursive,
                ) {
                    error!(
                        ?err,
                        "Unable to watch folder {}",
                        folder_monitor.source_folder
                    );
                } else {
                    watched_folders.push(PathBuf::from(folder_monitor.source_folder.clone()));
                }
            }
        }
    }

    #[tracing::instrument]
    fn free_watchers(watched_folders: &mut Vec<PathBuf>, watcher: &mut INotifyWatcher) {
        for folder in watched_folders.iter() {
            if let Err(err) = watcher.unwatch(folder) {
                warn!(
                    ?err,
                    "Unable to unwatch folder {}",
                    folder.display()
                );
            }
        }

        watched_folders.clear();
    }

    pub fn monitor_folders(
        config: Arc<RwLock<FileHandlingConfig>>,
        tx_new_file_event: Sender<PathBuf>,
        rx_config_updated: Receiver<()>,
    ) {
        let (tx, rx) = mpsc::channel::<PathBuf>();
        let mut watcher = match App::setup_watcher(tx) {
            Ok(watcher) => watcher,
            Err(err) => panic!("Unable to generate watcher: {err:#?}"),
        };

        let mut watched_folders = vec![];
        App::setup_folders(&config, &mut watcher, &mut watched_folders);

        loop {
            if let Ok(filename) = rx.try_recv() {
                if let Err(err) = tx_new_file_event.send(filename) {
                    error!(?err, "Unable to send filename over");
                }
            }

            if let Ok(_) = rx_config_updated.try_recv() {
                App::free_watchers(&mut watched_folders, &mut watcher);
                App::setup_folders(&config, &mut watcher, &mut watched_folders);
            }

            thread::sleep(Duration::from_millis(100));
        }
    }
}
