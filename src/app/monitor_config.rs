use std::{
    path::{Path, PathBuf},
    sync::{
        Arc, Mutex,
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
    config::Config,
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

    pub fn monitor_config(config: Arc<Mutex<Config>>, config_path: PathBuf, tx_config_updated: Sender<()>) {
        let (sender, receiver) = mpsc::channel();
        let mut watcher = match App::setup_config_watcher(sender) {
            Ok(watcher) => watcher,
            Err(err) => panic!("Unable to generate watcher for config: {err:?}"),
        };

        if let Err(err) = watcher.watch(Path::new(&config_path), RecursiveMode::NonRecursive) {
            panic!("Unable to watch config file: {err:?}");
        }

        for _ in receiver {
            match config.lock() {
                Ok(mut config) => match confy::load_path::<Config>(&config_path) {
                    Ok(config_updated) => {
                        *config = config_updated;
                        trace!(?config, "Configuration updated");
                    }
                    Err(err) => error!(?err, "Unable to load configuration"),
                },
                Err(err) => error!(?err, "Unable to lock config"),
            }

            if let Err(err) = tx_config_updated.send(()) {
                error!(?err, "Unable to send notification of configuration update");
            }
        }
    }
}
