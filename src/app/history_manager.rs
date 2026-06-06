use std::{fs::File, io::{BufWriter, Write}, path::PathBuf, sync::{Arc, Mutex, RwLock, mpsc::Receiver}};

use serde::{Deserialize, Serialize};

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
    history_filepath: PathBuf,
    history_file_max_size_mb: usize
}

impl HistoryManager {
    pub fn init(self, config: Arc<RwLock<HistoryConfig>>) -> Self {
        if let Ok(config) = config.read() {
            Self {
                history_filepath: config.filepath.clone(),
                history_file_max_size_mb: config.max_size_mb,
            }
        } else {
            panic!()
        }
    }

    pub fn run(mut self, rx_event: Receiver<EventType>) -> anyhow::Result<()> {
        let file = File::create(self.history_filepath)?;
        let mut writer = BufWriter::new(file);

        loop {
            if let Ok(event) = rx_event.try_recv() {
                
            }
        }
    }
}