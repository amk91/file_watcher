use std::{
    path::{Path, PathBuf},
    sync::{
        Arc, Mutex,
        mpsc::{self, Sender},
    },
};

use anyhow::anyhow;
use log::{error, warn};
use notify::{Event, EventKind, INotifyWatcher, RecursiveMode, Watcher, event::CreateKind};

use crate::{app::App, config::Config};

impl App {
    fn setup_watcher(sender: Sender<PathBuf>) -> notify::Result<INotifyWatcher> {
        notify::recommended_watcher(move |event: notify::Result<Event>| {
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

    pub fn monitor_folders(config: Arc<Mutex<Config>>, sender: Sender<PathBuf>) {
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
}
