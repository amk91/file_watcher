use std::{
    fs::{OpenOptions, metadata}, io::{BufWriter, Write}, path::PathBuf, sync::{Arc, RwLock}, time::Instant
};

use crossbeam_channel::{Receiver, select};
use serde::{Deserialize, Serialize};
use tracing::{error, trace};

use crate::config::HistoryConfig;

#[derive(Debug, Default, Serialize, Deserialize)]
pub struct FileEventInfo {
    pub filepath: PathBuf,
    pub destination_path: PathBuf,
}

#[derive(Debug, Serialize, Deserialize)]
pub enum EventType {
    FileDetected(FileEventInfo),
    FileMoved(FileEventInfo),
    SouceFolderMissing(String),
    SourceFileMissing(PathBuf),
    ConfigUpdated(String),
}

#[derive(Debug, Default)]
pub struct HistoryManager {
    config: Arc<RwLock<HistoryConfig>>,
}

impl HistoryManager {
    pub fn init(self, config: Arc<RwLock<HistoryConfig>>) -> Self {
        Self { config }
    }

    pub fn run(
        self,
        rx_event: Receiver<EventType>,
        rx_config_update: Receiver<()>,
    ) -> anyhow::Result<()> {
        let config = self.config.read().expect("Unable to acquire lock");
        let mut filepath = config.filepath.clone();
        let mut max_size_mb = config.max_size_mb;
        let mut flush_interval = config.flush_interval;
        trace!(?config);
        drop(config);

        let mut recreate_file = false;
        let mut writer = BufWriter::new(
            OpenOptions::new()
                .create(true)
                .append(true)
                .open(&filepath)?,
        );

        let mut next_flush = Instant::now() + flush_interval;
        loop {
            let time_to_next_flush = next_flush.saturating_duration_since(Instant::now());
            select! {
                recv(rx_config_update) -> _ => {
                    if let Ok(config) = self.config.read() {
                        recreate_file = filepath != config.filepath;
                        filepath = config.filepath.clone();
                        max_size_mb = config.max_size_mb;
                        flush_interval = config.flush_interval;
                        trace!(
                            ?recreate_file,
                            ?filepath,
                            ?max_size_mb,
                            ?flush_interval,
                            "Configuration updated"
                        );
                    }
                },

                recv(rx_event) -> event => {
                    if let Ok(event) = event {
                        match serde_json::to_string(&event) {
                            Ok(mut event_json) => {
                                trace!(?event, "Event received");
                                event_json.push('\n');
                                if let Err(err) = writer.write(event_json.as_bytes()) {
                                    error!(?err, "Unable to write to BufWriter the event: {event_json}");
                                }
                            }
                            Err(err) => error!(?err, ?event, "Unable to convert event to json"),
                        }
                    }
                }

                recv(crossbeam_channel::after(time_to_next_flush)) -> _ => {
                    if let Err(err) = writer.flush() {
                        error!(?err, "Unable to flush content to history file");
                    }

                    if recreate_file {
                        writer = BufWriter::new(
                            OpenOptions::new().create(true).append(true).open(&filepath)?
                        );
                        recreate_file = false;
                    }

                    match metadata(&filepath) {
                        Ok(metadata) => {
                            let max_size_bytes = (max_size_mb as u64) * 1024 * 1024;
                            if metadata.len() >= max_size_bytes {
                                let bak_filepath = filepath.clone().with_extension("bak");
                                std::fs::rename(&filepath, &bak_filepath)?;

                                writer = BufWriter::new(
                                    OpenOptions::new().create(true).append(true).open(&filepath)?
                                );
                            }
                        },
                        Err(err) => error!(
                            ?err,
                            "Unable to retrieve metadata for file {}",
                            filepath.display()
                        ),
                    }

                    next_flush = Instant::now() + flush_interval;
                }
            }
        }
    }
}
