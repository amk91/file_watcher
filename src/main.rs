use inotify::{EventMask, Inotify, WatchMask};
use lazy_static::lazy_static;
use log::{error, info, trace, warn};

use std::{
    collections::HashMap,
    fs, io,
    path::Path,
    sync::mpsc::{self, Receiver, Sender},
    thread,
    time::{Duration, SystemTime},
};

mod config;
use config::Config;

lazy_static! {
    static ref CONFIG: Config = confy::load("file_janitor", Some("config")).unwrap();
}

fn main() {
    pretty_env_logger::init();
    trace!("log initialized");

    match fs::exists(&CONFIG.monitored_folder) {
        Ok(false) | Err(_) => panic!("Folder {} does not exists", &CONFIG.monitored_folder),
        _ => {}
    }

    match std::fs::create_dir_all(&CONFIG.destination_folder) {
        Ok(_) => trace!("Destination folder {} created", CONFIG.destination_folder),
        Err(err) if err.kind() == io::ErrorKind::AlreadyExists => trace!("Folder alread exists"),
        Err(err) => panic!(
            "Unable to create {} - err: {err:?}",
            CONFIG.destination_folder
        ),
    }

    let (sender, receiver) = mpsc::channel::<String>();

    let monitor_folder_thread = thread::spawn(|| {
        monitor_folder(sender);
    });

    let handle_files_thread = thread::spawn(|| {
        handle_files(receiver);
    });

    monitor_folder_thread.join().unwrap();
    handle_files_thread.join().unwrap();
}

fn monitor_folder(sender: Sender<String>) {
    let mut inotify = Inotify::init().expect("Error while initializing inotify instance");

    inotify
        .watches()
        .add(&CONFIG.monitored_folder, WatchMask::CREATE)
        .expect("Failed to add file watch");

    let mut buffer = [0u8; 4096];
    loop {
        let events = inotify
            .read_events_blocking(&mut buffer)
            .expect("Error while reading events");

        for event in events {
            if !event.mask.contains(EventMask::ISDIR) {
                if let Some(name) = event.name.map_or(None, |n| n.to_str()) {
                    if let Err(err) = sender.send(name.into()) {
                        error!("Err: {err:?}");
                    }
                }
            }
        }
    }
}

fn handle_files(receiver: Receiver<String>) {
    let mut files_list: HashMap<String, (Duration, u8)> = HashMap::new();
    let mut timeout = CONFIG.check_interval;
    let mut process_time = SystemTime::now();
    loop {
        if let Ok(name) = receiver.try_recv() {
            let mut name_parts = name.rsplit('.');
            trace!("event received {name}");

            let mut next = name_parts.next();
            while let Some(section) = next {
                if CONFIG.part_temp_file_check && section == "part" {
                    next = name_parts.next();
                    trace!("part file found: {name}");
                }

                let section = String::from(section);
                if CONFIG.extensions.contains(&section) {
                    trace!("{section} file found: {name}");

                    let mut filename = name_parts.collect::<Vec<&str>>();
                    filename.reverse();
                    let filename = format!("{0}.{section}", filename.join("."));

                    if files_list.contains_key(&filename) {
                        files_list.remove(&filename);
                        trace!("possible temp file {name} already registered and will be removed");
                    } else {
                        files_list.insert(filename, (CONFIG.file_timeout, CONFIG.move_attempts));
                        trace!("possible temp file {name} not present in hash map, register");
                    }

                    break;
                }
            }
        }

        timeout = timeout.saturating_sub(
            CONFIG
                .thread_sleep
                .saturating_sub(process_time.elapsed().unwrap_or(Duration::ZERO)),
        );

        if timeout == Duration::ZERO {
            timeout = CONFIG.check_interval;
            let mut files_to_move = vec![];
            for (file, (duration, _)) in files_list.iter_mut() {
                *duration = duration.saturating_sub(CONFIG.check_interval);
                if *duration == Duration::ZERO {
                    files_to_move.push(file.clone());
                    trace!("file {file} will be removed from hashmap");
                }
            }

            for file in files_to_move {
                match move_file(&file) {
                    Ok(_) => {
                        files_list.remove(&file);
                        trace!(
                            "file {file} moved successfully and it will be removed from the list of monitored files"
                        );
                    }
                    Err(_) => {
                        match files_list.get_mut(&file) {
                            Some((_, 0)) => {
                                files_list.remove(&file);
                                warn!(
                                    "Unable to move file {file} to {}",
                                    CONFIG.destination_folder
                                );
                            }
                            Some((_, attempts)) => {
                                *attempts = attempts.strict_sub(1);
                            }
                            None => {}
                        };
                    }
                }
            }
        }

        thread::sleep(CONFIG.thread_sleep);
        process_time = SystemTime::now();
    }
}

fn move_file(file: &str) -> io::Result<()> {
    let destination_filepath = Path::new(&CONFIG.destination_folder).join(file);
    if fs::exists(&destination_filepath)? {
        error!("Destionation file {destination_filepath:?} already exists");
        return Err(io::Error::new(
            io::ErrorKind::AlreadyExists,
            format!("Destination file {destination_filepath:?} already exists"),
        ));
    }

    let source_filepath = Path::new(&CONFIG.monitored_folder).join(file);
    if !fs::exists(&source_filepath)? {
        error!("Source file {source_filepath:?} not found");
        return Err(io::Error::new(
            io::ErrorKind::NotFound,
            format!("Source file {source_filepath:?} not found"),
        ));
    }

    match fs::rename(&source_filepath, &destination_filepath) {
        Ok(_) => {
            info!(
                "File {file} successfully moved from {} to {}",
                CONFIG.monitored_folder, CONFIG.destination_folder,
            );
            Ok(())
        }
        Err(err) if err.kind() == io::ErrorKind::CrossesDevices => {
            info!("Cross device transfer not allowed, copy-remove data");
            fs::copy(&source_filepath, &destination_filepath).map(|_| ())?;
            fs::remove_file(&source_filepath)
        }
        Err(err) => Err(err),
    }
}
