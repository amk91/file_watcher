use std::{
    fs::File,
    io::{BufWriter, Write},
    sync::{Arc, RwLock, mpsc::Receiver},
    thread,
    time::{Duration, SystemTime},
};

use serde::{Deserialize, Serialize};
use tracing::error;

use crate::config::HistoryConfig;

#[derive(Debug, Default, Serialize, Deserialize)]
struct FileEventInfo {
    filepath: String,
    destination_path: String,
}

#[derive(Debug, Serialize, Deserialize)]
enum EventType {
    FileDetected(FileEventInfo),
    FileMoved(FileEventInfo),
    SouceFolderMissing(String),
}

#[derive(Debug, Default)]
struct HistoryManager {
    config: Arc<RwLock<HistoryConfig>>,
}

impl HistoryManager {
    pub fn init(self, config: Arc<RwLock<HistoryConfig>>) -> Self {
        Self { config }
    }

    pub fn run(
        mut self,
        rx_event: Receiver<EventType>,
        rx_config_update: Receiver<()>,
    ) -> anyhow::Result<()> {
        let config = self.config.read().expect("Unable to acquire lock");
        let mut filepath = config.filepath.clone();
        let mut max_size_mb = config.max_size_mb;
        let mut flush_interval = config.flush_interval;
        let mut thread_sleep = config.thread_sleep;
        drop(config);

        let mut file = File::create(&filepath)?;
        let mut writer = BufWriter::new(file);

        let mut timeout = flush_interval;
        let mut process_time = SystemTime::now();
        loop {
            if let Ok(_) = rx_config_update.try_recv() {
                if let Ok(config) = self.config.read() {
                    filepath = config.filepath.clone();
                    max_size_mb = config.max_size_mb;
                    flush_interval = config.flush_interval;
                    thread_sleep = config.thread_sleep;
                }

                file = File::create(&filepath)?;

                //TODO: if there are events to be logged at this point they are lost, fix it!
                writer = BufWriter::new(file);
            }

            if let Ok(event) = rx_event.try_recv() {
                match serde_json::to_string(&event) {
                    Ok(event) => {
                        if let Err(err) = writer.write(event.as_bytes()) {
                            error!(?err, "Unable to write to BufWriter the event: {event}");
                        }
                    }
                    Err(err) => error!(?err, ?event, "Unable to convert event to json"),
                }
            }

            timeout = timeout.saturating_sub(
                thread_sleep.saturating_sub(process_time.elapsed().unwrap_or(Duration::ZERO)),
            );

            if timeout == Duration::ZERO {
                timeout = flush_interval;

                //TODO: check size of file, if less than max flush, otherwise switch before doing so
            }

            thread::sleep(Duration::from_millis(100));
            process_time = SystemTime::now();
        }
    }
}
