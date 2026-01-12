#![allow(dead_code)]
use std::{
    fs::{File, OpenOptions},
    io::Write,
    path::Path,
    sync::Mutex,
    time::{Instant},
};

pub struct SimpleLogger {
    file: Mutex<File>,
    logger_created: Instant
}

impl SimpleLogger {
    pub fn new(log_path: &Path) -> Self {
        let file = OpenOptions::new()
            .create(true)
            .write(true)
            .open(log_path)
            .expect("Failed to open log file");
        SimpleLogger {
            file: Mutex::new(file),
            logger_created: Instant::now()
        }
    }

    pub fn log(&self, level: &str, message: &str) {
        let mut file = self.file.lock().unwrap();
        let uptime = self.logger_created.elapsed().as_millis();
        writeln!(file, "{} {} - {}", uptime, level, message).expect("Failed to write to log file");
    }

    pub fn log_info(&self, message: &str) {
        self.log("INFO", message);
    }

    pub fn log_debug(&self, message: &str) {
        self.log("DEBUG", message);
    }
}