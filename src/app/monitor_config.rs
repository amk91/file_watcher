use std::{
    path::{Path, PathBuf},
    sync::{
        Arc, RwLock,
        mpsc::{self, Sender},
    },
};

use notify::{
    Event, EventKind, INotifyWatcher, RecursiveMode, Watcher,
    event::{AccessKind, AccessMode},
};
use tracing::{error, trace};

use crate::{
    app::App,
    config::{Config, FileHandlingConfig},
};

impl App {
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

    pub fn monitor_config(
        config_path: PathBuf,
        tx_config_updated: Sender<()>,
        file_handling_config: Arc<RwLock<FileHandlingConfig>>,
    ) {
        let (sender, receiver) = mpsc::channel();
        let mut watcher = match App::setup_config_watcher(sender) {
            Ok(watcher) => watcher,
            Err(err) => panic!("Unable to generate watcher for config: {err:?}"),
        };

        if let Err(err) = watcher.watch(Path::new(&config_path), RecursiveMode::NonRecursive) {
            panic!("Unable to watch config file: {err:?}");
        }

        for _ in receiver {
            trace!("Configuration has been changed");
            if let Ok(mut file_handling_config) = file_handling_config.write() {
                match confy::load_path::<Config>(&config_path) {
                    Ok(config_updated) => {
                        *file_handling_config = config_updated.file_handling_config;
                    }
                    Err(err) => error!(?err, "Unable to load configuration"),
                }
            }

            if let Err(err) = tx_config_updated.send(()) {
                error!(?err, "Unable to send notification of configuration update");
            }
        }
    }
}
