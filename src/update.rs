//! "A new version is available": once a day the engine asks GitHub for the latest release
//! (one HTTPS request, nothing about the PC is sent) and the tray + dashboard say so.

use crate::ipc::{UpdateInfo, UpdateStatus};
use serde::{Deserialize, Serialize};
use std::sync::{Arc, Mutex};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

pub const REPO: &str = "Avilash2001/avix";
const EVERY: Duration = Duration::from_secs(24 * 3600);
/// Don't hit the network right at sign-in.
const FIRST_DELAY: Duration = Duration::from_secs(60);

fn now() -> u64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0)
}

/// The version this build reports (debug builds can pretend to be older, for testing).
pub fn current_version() -> String {
    #[cfg(debug_assertions)]
    if let Ok(v) = std::env::var("AVIX_FAKE_VERSION") {
        return v;
    }
    crate::VERSION.to_string()
}

/// "v1.2.3" / "1.2" -> (1, 2, 3); anything unparsable -> None.
pub fn parse(v: &str) -> Option<(u32, u32, u32)> {
    let v = v.trim().trim_start_matches(['v', 'V']);
    let core = v.split(['-', '+']).next()?;
    let mut it = core.split('.').map(|x| x.parse::<u32>());
    let a = it.next()?.ok()?;
    let b = it.next().unwrap_or(Ok(0)).ok()?;
    let c = it.next().unwrap_or(Ok(0)).ok()?;
    Some((a, b, c))
}

pub fn is_newer(latest: &str, current: &str) -> bool {
    matches!((parse(latest), parse(current)), (Some(l), Some(c)) if l > c)
}

#[derive(Default, Serialize, Deserialize)]
#[serde(default)]
struct Saved {
    checked_at: u64,
    latest: Option<UpdateInfo>,
    /// newest version the tray already announced (so it says it once)
    announced: String,
}

fn path() -> std::path::PathBuf {
    crate::sys::data_dir().join("update.json")
}

fn load() -> Saved {
    std::fs::read_to_string(path()).ok().and_then(|t| serde_json::from_str(&t).ok()).unwrap_or_default()
}

fn save(s: &Saved) {
    if let Ok(t) = serde_json::to_string_pretty(s) {
        let _ = std::fs::write(path(), t);
    }
}

/// Ask GitHub for the latest (non-pre-release) version.
pub fn fetch_latest() -> Result<UpdateInfo, String> {
    let body = crate::sys::http::get(
        "api.github.com",
        &format!("/repos/{REPO}/releases/latest"),
        "Accept: application/vnd.github+json\r\n",
        512 * 1024,
    )?;
    let v: serde_json::Value = serde_json::from_str(&body).map_err(|e| format!("unexpected reply: {e}"))?;
    let tag = v.get("tag_name").and_then(|x| x.as_str()).ok_or("no version in reply")?;
    let url = v
        .get("html_url")
        .and_then(|x| x.as_str())
        .filter(|u| u.starts_with(&format!("https://github.com/{REPO}/")))
        .map(str::to_string)
        .unwrap_or_else(|| format!("https://github.com/{REPO}/releases/latest"));
    let version = tag.trim_start_matches(['v', 'V']).to_string();
    parse(&version).ok_or_else(|| format!("odd version '{tag}'"))?;
    Ok(UpdateInfo { version, url })
}

/// Lives in the engine: schedules checks on a background thread and remembers the answer.
pub struct Checker {
    saved: Saved,
    started: std::time::Instant,
    running: Option<Arc<Mutex<Option<Result<UpdateInfo, String>>>>>,
    error: Option<String>,
}

impl Checker {
    pub fn new() -> Self {
        Self { saved: load(), started: std::time::Instant::now(), running: None, error: None }
    }

    /// Call every tick. Returns a newly found version that hasn't been announced yet.
    pub fn tick(&mut self, enabled: bool, force: bool) -> Option<UpdateInfo> {
        if let Some(slot) = &self.running {
            let done = slot.lock().ok().and_then(|mut g| g.take());
            if let Some(r) = done {
                self.running = None;
                self.saved.checked_at = now();
                match r {
                    Ok(info) => {
                        self.error = None;
                        self.saved.latest = Some(info);
                    }
                    Err(e) => self.error = Some(e),
                }
                save(&self.saved);
            }
        } else if force
            || (enabled
                && self.started.elapsed() >= FIRST_DELAY
                && now().saturating_sub(self.saved.checked_at) >= EVERY.as_secs())
        {
            let slot = Arc::new(Mutex::new(None));
            let s2 = slot.clone();
            let spawned = std::thread::Builder::new().name("avix-update".into()).stack_size(256 * 1024).spawn(move || {
                let r = fetch_latest();
                if let Ok(mut g) = s2.lock() {
                    *g = Some(r);
                }
            });
            if spawned.is_ok() {
                self.running = Some(slot);
            }
        }
        let avail = self.available()?;
        if self.saved.announced != avail.version {
            self.saved.announced = avail.version.clone();
            save(&self.saved);
            return Some(avail);
        }
        None
    }

    /// A newer version than this one, if the last check found it.
    pub fn available(&self) -> Option<UpdateInfo> {
        self.saved.latest.clone().filter(|l| is_newer(&l.version, &current_version()))
    }

    pub fn status(&self) -> UpdateStatus {
        UpdateStatus {
            current: current_version(),
            available: self.available(),
            checked_secs_ago: (self.saved.checked_at > 0).then(|| now().saturating_sub(self.saved.checked_at)),
            checking: self.running.is_some(),
            error: self.error.clone(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn versions() {
        assert_eq!(parse("v0.1.3"), Some((0, 1, 3)));
        assert_eq!(parse("1.2"), Some((1, 2, 0)));
        assert_eq!(parse("v2.0.0-beta.1"), Some((2, 0, 0)));
        assert_eq!(parse("latest"), None);
        assert!(is_newer("0.1.3", "0.1.2"));
        assert!(is_newer("v0.2.0", "0.1.9"));
        assert!(is_newer("1.0.0", "0.9.9"));
        assert!(!is_newer("0.1.2", "0.1.2"));
        assert!(!is_newer("0.1.1", "0.1.2"));
        assert!(!is_newer("junk", "0.1.2"));
    }
}
