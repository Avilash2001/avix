//! Crash-safety: while a boost is active, everything needed to undo it is written
//! here. If Avix is killed mid-boost, the next start (or `avix --restore`) undoes it.

use super::Touched;
use crate::logging::log;
use crate::sys::{self, power, services};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct Journal {
    pub prev_plan: Option<String>,
    pub plan_applied: Option<String>,
    pub stopped_services: Vec<String>,
    pub touched: Vec<Touched>,
    pub timer: bool,
}

fn path() -> PathBuf {
    sys::data_dir().join("boost-journal.json")
}

pub fn write(j: &Journal) {
    let Ok(text) = serde_json::to_string(j) else { return };
    let tmp = path().with_extension("json.tmp");
    if std::fs::write(&tmp, text).is_ok() {
        let _ = std::fs::rename(&tmp, path());
    }
}

pub fn clear() {
    let _ = std::fs::remove_file(path());
}

pub fn restore_from_disk() {
    let Ok(text) = std::fs::read_to_string(path()) else { return };
    let Ok(j) = serde_json::from_str::<Journal>(&text) else {
        clear();
        return;
    };
    log("Restoring settings left behind by an interrupted boost");
    let restored = j.touched.iter().filter(|t| t.restore()).count();
    for s in &j.stopped_services {
        let _ = services::start(s);
    }
    if let (Some(prev), Some(applied)) =
        (j.prev_plan.as_deref().and_then(power::parse), j.plan_applied.as_deref().and_then(power::parse))
    {
        if power::active().is_some_and(|cur| power::same(&cur, &applied)) {
            let _ = power::set_active(&prev);
        }
    }
    log(&format!(
        "Restore complete: {restored} processes, {} services, power plan",
        j.stopped_services.len()
    ));
    clear();
}
