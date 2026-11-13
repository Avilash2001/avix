//! "Run this game on the dedicated GPU": the per-app setting in Windows' Settings > Display >
//! Graphics. On laptops with integrated + dedicated graphics, games sometimes start on the weak
//! integrated GPU. Avix sets the preference for games it boosts, never overrides a choice the user
//! made, and remembers what it set so it can take it back (gpu_prefs.json).

use crate::sys::registry::{self, Hive, RegValue};

const KEY: &str = r"Software\Microsoft\DirectX\UserGpuPreferences";
const HIGH_PERFORMANCE: &str = "GpuPreference=2;";

fn store() -> std::path::PathBuf {
    crate::sys::data_dir().join("gpu_prefs.json")
}

fn load() -> Vec<String> {
    std::fs::read_to_string(store()).ok().and_then(|t| serde_json::from_str(&t).ok()).unwrap_or_default()
}

fn save(v: &[String]) {
    if v.is_empty() {
        let _ = std::fs::remove_file(store());
    } else if let Ok(t) = serde_json::to_string_pretty(v) {
        let _ = std::fs::write(store(), t);
    }
}

/// Prefer the high-performance GPU for this exe. Returns true only if Avix set it just now
/// (false when the user already chose something for it, or it was set before).
pub fn prefer_dedicated(exe_path: &str) -> bool {
    if exe_path.len() < 4 || registry::get(Hive::HKCU, KEY, exe_path).is_some() {
        return false;
    }
    if registry::set(Hive::HKCU, KEY, exe_path, &RegValue::Str(HIGH_PERFORMANCE.into())).is_err() {
        return false;
    }
    let mut v = load();
    if !v.iter().any(|p| p.eq_ignore_ascii_case(exe_path)) {
        v.push(exe_path.to_string());
        save(&v);
    }
    true
}

/// Games Avix set the preference for (and that still have Avix's value).
pub fn set_by_avix() -> Vec<String> {
    load()
        .into_iter()
        .filter(|p| registry::get_string(Hive::HKCU, KEY, p).is_some_and(|v| v.eq_ignore_ascii_case(HIGH_PERFORMANCE)))
        .collect()
}

/// Remove every preference Avix set (ones the user changed since are left alone).
pub fn remove_all() -> usize {
    let mut n = 0;
    for p in load() {
        if registry::get_string(Hive::HKCU, KEY, &p).is_some_and(|v| v.eq_ignore_ascii_case(HIGH_PERFORMANCE))
            && registry::delete(Hive::HKCU, KEY, &p).is_ok()
        {
            n += 1;
        }
    }
    save(&[]);
    n
}
