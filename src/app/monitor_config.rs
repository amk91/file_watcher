use std::{
    path::{Path, PathBuf},
    sync::{Arc, RwLock},
};

use crossbeam_channel::{Sender, unbounded};
use notify::{
    Event, EventKind, INotifyWatcher, RecursiveMode, Watcher,
    event::{AccessKind, AccessMode},
};
use tracing::{Level, error, span, trace, warn};

use crate::{
    app::{App, history_manager::EventType},
    config::{self, Config, FileHandlingConfig, HistoryConfig},
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
            trace!("Configuration has been changed");
            let (file_handling_config_updated, history_config_updated) =
                match confy::load_path::<Config>(&config_path) {
                    Ok(config_updated) => (
                        config_updated.file_handling_config,
                        config_updated.history_config,
                    ),
                    Err(err) => {
                        error!(
                            ?err,
                            "Unable to load configuration at path {}",
                            config_path.display()
                        );
                        continue;
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

    fn notify_config<T: PartialEq>(
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

                    if let Err(err) = tx_config_update.send(()) {
                        error!(?err, "Unable to send notification about config updated");
                    }
                }
                Err(err) => warn!(?err, "Unable to lock config while changing the config"),
            }
        }
    }
}
