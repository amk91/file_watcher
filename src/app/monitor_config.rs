use std::{
    fs::File, io::Read, path::{Path, PathBuf}, sync::{Arc, RwLock}
};

use crossbeam_channel::{Sender, unbounded};
use notify::{
    Event, EventKind, INotifyWatcher, RecursiveMode, Watcher,
    event::{AccessKind, AccessMode},
};
use serde::Serialize;
use tracing::{error, info, warn};

use crate::{
    app::{App, history_manager::EventType},
    config::{Config, FileHandlingConfig, HistoryConfig},
};

impl App {
    pub fn monitor_config(
        config_path: PathBuf,
        tx_file_handling_config_updated: Sender<()>,
        tx_history_config_updated: Sender<()>,
        file_handling_config: Arc<RwLock<FileHandlingConfig>>,
        history_config: Arc<RwLock<HistoryConfig>>,
        tx_event: Sender<EventType>,
    ) {
        let (sender, receiver) = unbounded::<()>();
        let mut watcher = match App::setup_config_watcher(sender) {
            Ok(watcher) => watcher,
            Err(err) => panic!("Unable to generate watcher for config: {err:?}"),
        };

        if let Err(err) = watcher.watch(Path::new(&config_path), RecursiveMode::NonRecursive) {
            panic!("Unable to watch config file: {err:?}");
        }

        for _ in receiver {
            info!("Configuration has been changed");
            let (file_handling_config_updated, history_config_updated) = {
                let mut file = match File::open(&config_path) {
                    Ok(file) => file,
                    Err(err) => {
                        warn!(?err, "Unable to open file at {}", config_path.display());
                        continue;
                    }
                };

                let mut config_buffer = String::new();
                if let Err(err) = file.read_to_string(&mut config_buffer) {
                    warn!(?err, "Unable to read the whole file to string, filepath {}", config_path.display());
                    continue;
                }

                match yaml_serde::from_str::<Config>(&config_buffer) {
                    Ok(config) => (config.file_handling_config, config.history_config),
                    Err(err) => {
                        warn!(?err, "Unable to parse configuration from yaml file at {}", config_path.display());
                        continue;
                    }
                }
            };

            App::notify_config(
                &file_handling_config,
                file_handling_config_updated,
                &tx_file_handling_config_updated,
                &tx_event
            );
            App::notify_config(
                &history_config,
                history_config_updated,
                &tx_history_config_updated,
                &tx_event
            );
        }
    }

    #[tracing::instrument]
    fn setup_config_watcher(sender: Sender<()>) -> notify::Result<INotifyWatcher> {
        notify::recommended_watcher(move |event: notify::Result<Event>| {
            if let Ok(event) = event {
                if event.kind == EventKind::Access(AccessKind::Close(AccessMode::Write)) {
                    if let Err(err) = sender.send(()) {
                        error!(?err, "Error sending event for config file");
                    }
                }
            }
        })
    }

    fn notify_config<T: PartialEq + Serialize + Sized + std::fmt::Debug>(
        config: &Arc<RwLock<T>>,
        config_updated: T,
        tx_config_update: &Sender<()>,
        tx_event: &Sender<EventType>,
    ) {
        let config_changed = {
            match config.read() {
                Ok(config) => *config != config_updated,
                Err(err) => {
                    warn!(?err, "Unable to lock config while checking for changes");
                    false
                }
            }
        };

        if config_changed {
            match config.write() {
                Ok(mut config) => {
                    *config = config_updated;

                    match serde_json::to_string(&*config) {
                        Ok(json_string) => if let Err(err) = tx_event.send(
                            EventType::ConfigUpdated(json_string.clone())
                        ) {
                            error!(?err, %json_string, "Unable to send event to history manager");
                        },
                        Err(err) => error!(?err, ?config, "Unable to convert config to json with serde"),
                    }

                    if let Err(err) = tx_config_update.send(()) {
                        error!(?err, "Unable to send notification about config updated");
                    }
                }
                Err(err) => warn!(?err, "Unable to lock config while changing the config"),
            }
        }
    }
}
