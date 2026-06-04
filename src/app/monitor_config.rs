use std::{
    path::Path,
    sync::{
        Arc, Mutex,
        mpsc::{self, Sender},
    },
};

use log::{error, trace};
use notify::{
    Event, EventKind, INotifyWatcher, RecursiveMode, Watcher,
    event::{AccessKind, AccessMode},
};

use crate::{
    app::App,
    config::{APP_NAME, CONFIG_NAME, Config},
};

impl App {
    fn setup_config_watcher(sender: Sender<()>) -> notify::Result<INotifyWatcher> {
        notify::recommended_watcher(move |event: notify::Result<Event>| {
            if let Ok(event) = event {
                if event.kind == EventKind::Access(AccessKind::Close(AccessMode::Write)) {
                    if let Err(err) = sender.send(()) {
                        error!("Error sending event for config file: {err:#?}");
                    }
                }
            }
        })
    }

    pub fn monitor_config(config: Arc<Mutex<Config>>, tx_config_updated: Sender<()>) {
        let (sender, receiver) = mpsc::channel();
        let mut watcher = match App::setup_config_watcher(sender) {
            Ok(watcher) => watcher,
            Err(err) => panic!("Unable to generate watcher for config: {err:?}"),
        };

        let config_path = match confy::get_configuration_file_path(APP_NAME, CONFIG_NAME) {
            Ok(config_path) => config_path,
            Err(err) => panic!("Unable to retrieve configuration filepath: {err:?}"),
        };

        if let Err(err) = watcher.watch(Path::new(&config_path), RecursiveMode::NonRecursive) {
            panic!("Unable to watch config file: {err:?}");
        }

        for _ in receiver {
            match config.lock() {
                Ok(mut config) => match confy::load::<Config>(APP_NAME, Some(CONFIG_NAME)) {
                    Ok(config_updated) => {
                        *config = config_updated;
                        trace!("Configuration updated: {config:#?}");
                    },
                    Err(err) => error!("Unable to load configuration: {err:?}"),
                },
                Err(err) => error!("Unable to lock config: {err:?}"),
            }

            tx_config_updated.send(()).unwrap();
        }
    }
}
