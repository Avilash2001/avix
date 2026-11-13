//! Tiny file logger: %LOCALAPPDATA%\Avix\avix-<role>.log (rotated at 1 MB).

use std::fs::{File, OpenOptions};
use std::io::Write;
use std::sync::Mutex;

static LOG: Mutex<Option<File>> = Mutex::new(None);

pub fn init(role: &str) {
    let path = crate::sys::data_dir().join(format!("avix-{role}.log"));
    if let Ok(meta) = std::fs::metadata(&path) {
        if meta.len() > 1024 * 1024 {
            let _ = std::fs::rename(&path, path.with_extension("log.old"));
        }
    }
    if let Ok(f) = OpenOptions::new().create(true).append(true).open(&path) {
        if let Ok(mut g) = LOG.lock() {
            *g = Some(f);
        }
    }
    log(&format!("---- Avix {} ({role}) started ----", crate::VERSION));
}

pub fn log(msg: &str) {
    if let Ok(mut g) = LOG.lock() {
        if let Some(f) = g.as_mut() {
            let _ = writeln!(f, "[{}] {}", crate::sys::local_datetime(), msg);
        }
    }
}
