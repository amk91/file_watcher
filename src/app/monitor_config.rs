use std::{
    fmt::Debug,
    fs::File,
    io::Read,
    path::{Path, PathBuf},
    sync::{Arc, RwLock},
};

use crossbeam_channel::{Sender, unbounded};
use notify::{
    Event, EventKind, INotifyWatcher, RecursiveMode, Watcher,
    event::{AccessKind, AccessMode},
};
use serde::Serialize;
use tracing::{error, info, warn};

use crate::{
    app::App,
    config::{Config, FileHandlingConfig, HistoryConfig},
    history_manager::{ConfigUpdatedType, EventType},
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
                    warn!(
                        ?err,
                        "Unable to read the whole file to string, filepath {}",
                        config_path.display()
                    );
                    continue;
                }

                match serde_json::from_str::<Config>(&config_buffer) {
                    Ok(mut config) => {
                        config.check_for_duplicate_monitors();
                        (config.file_handling_config, config.history_config)
                    }
                    Err(err) => {
                        warn!(
                            ?err,
                            "Unable to parse configuration from yaml file at {}",
                            config_path.display()
                        );
                        continue;
                    }
                }
            };

            App::notify_config(
                &file_handling_config,
                file_handling_config_updated,
                &tx_file_handling_config_updated,
                &tx_event,
                ConfigUpdatedType::FileHandling,
            );
            App::notify_config(
                &history_config,
                history_config_updated,
                &tx_history_config_updated,
                &tx_event,
                ConfigUpdatedType::History,
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

    fn notify_config<T, F>(
        config: &Arc<RwLock<T>>,
        config_updated: T,
        tx_config_update: &Sender<()>,
        tx_event: &Sender<EventType>,
        to_config_updated: F,
    ) where
        T: PartialEq + Serialize + Sized + Debug + Clone,
        F: FnOnce(T) -> ConfigUpdatedType,
    {
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

                    let event = EventType::ConfigUpdated(to_config_updated((*config).clone()));
                    if let Err(err) = tx_event.send(event) {
                        error!(?err, "Unable to send event to history manager");
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
