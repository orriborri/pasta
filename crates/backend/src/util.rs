use chrono::Local;
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::PathBuf;

pub fn log_dir() -> PathBuf {
    let dir = PathBuf::from("./logs");
    fs::create_dir_all(&dir).ok();
    dir
}

pub fn log(agent: &str, msg: &str) {
    let path = log_dir().join("scheduler.log");
    if let Ok(mut f) = OpenOptions::new().create(true).append(true).open(path) {
        writeln!(f, "[{}] [{}] {}", Local::now().format("%Y-%m-%d %H:%M:%S"), agent, msg).ok();
    }
}

pub fn get_rss_mb() -> f64 {
    fs::read_to_string("/proc/self/statm")
        .ok()
        .and_then(|s| s.split_whitespace().nth(1)?.parse::<u64>().ok())
        .map(|pages| pages as f64 * 4096.0 / 1_048_576.0)
        .unwrap_or(0.0)
}
