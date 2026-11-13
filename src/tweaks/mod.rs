//! Reversible system tweaks. Every change records the original value first
//! (%LOCALAPPDATA%\Avix\tweak_backup.json), so each tweak can be reverted exactly.

pub mod apps;
pub mod catalog;
pub mod gpu_pref;
pub mod startup;
pub mod tasks;

use crate::sys::registry::{self, Hive, RegValue};
use crate::sys::services;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Category {
    Gaming,
    Performance,
    Network,
    Privacy,
    Ads,
    Services,
}

impl Category {
    pub const ALL: [Category; 6] = [
        Category::Gaming,
        Category::Performance,
        Category::Network,
        Category::Privacy,
        Category::Ads,
        Category::Services,
    ];

    pub fn label(self) -> &'static str {
        match self {
            Category::Gaming => "Gaming",
            Category::Performance => "Performance",
            Category::Network => "Network",
            Category::Privacy => "Privacy & AI",
            Category::Ads => "Ads & nags",
            Category::Services => "Services",
        }
    }

    pub fn blurb(self) -> &'static str {
        match self {
            Category::Gaming => "Settings that directly affect frame pacing and input feel in games.",
            Category::Performance => "System-wide scheduling and power behaviour.",
            Category::Network => "Background bandwidth and latency-related settings.",
            Category::Privacy => "Telemetry, tracking, Copilot and Recall.",
            Category::Ads => "Start menu ads, auto-installed apps and nag screens.",
            Category::Services => "Background services most gaming PCs don't need.",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum Risk {
    Low,
    Medium,
    High,
}

impl Risk {
    pub fn label(self) -> &'static str {
        match self {
            Risk::Low => "Safe",
            Risk::Medium => "Moderate",
            Risk::High => "Advanced",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Restart {
    None,
    Explorer,
    SignOut,
    Reboot,
}

impl Restart {
    pub fn label(self) -> Option<&'static str> {
        match self {
            Restart::None => None,
            Restart::Explorer => Some("Explorer restart"),
            Restart::SignOut => Some("sign-out"),
            Restart::Reboot => Some("reboot"),
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub enum V {
    D(u32),
    S(&'static str),
}

impl V {
    pub fn reg(self) -> RegValue {
        match self {
            V::D(d) => RegValue::Dword(d),
            V::S(s) => RegValue::Str(s.to_string()),
        }
    }
}

#[derive(Clone, Debug)]
pub enum Action {
    /// Registry value; `default` is the Windows default used when no backup exists (None = absent).
    Reg { hive: Hive, path: &'static str, name: &'static str, value: V, default: Option<V> },
    /// One "Key=Value;" entry inside a string of such entries (e.g. DirectX's global settings),
    /// leaving the other entries alone.
    RegFlag { hive: Hive, path: &'static str, name: &'static str, key: &'static str, value: &'static str },
    /// Same DWORD on every TCP/IP interface key (default: absent).
    RegTcpInterfaces { name: &'static str, value: u32 },
    /// Service start type (`default` = Windows default).
    Service { name: &'static str, start: u32, default: u32 },
    /// Scheduled task to disable.
    Task { path: &'static str },
}

pub struct Tweak {
    pub id: &'static str,
    pub category: Category,
    pub title: &'static str,
    pub description: &'static str,
    pub risk: Risk,
    pub recommended: bool,
    pub restart: Restart,
    pub actions: Vec<Action>,
    /// Applies the change live where Windows needs a nudge (e.g. mouse settings).
    pub live: Option<fn()>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TweakState {
    Applied,
    Partial,
    NotApplied,
    NotApplicable,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum Saved {
    Reg {
        hive: Hive,
        path: String,
        name: String,
        value: Option<RegValue>,
        /// Keys that didn't exist before Avix wrote this value (deepest first).
        #[serde(default)]
        created: Vec<String>,
    },
    Service {
        name: String,
        start: u32,
        delayed: bool,
        #[serde(default)]
        running: Option<bool>,
    },
    Task { path: String, enabled: bool },
}

fn save_reg(hive: Hive, path: &str, name: &str) -> Saved {
    Saved::Reg {
        hive,
        path: path.to_string(),
        name: name.to_string(),
        value: registry::get(hive, path, name),
        created: registry::missing_keys(hive, path),
    }
}

#[derive(Default, Serialize, Deserialize)]
pub struct Backup {
    pub entries: HashMap<String, Vec<Saved>>,
}

impl Backup {
    fn path() -> std::path::PathBuf {
        crate::sys::data_dir().join("tweak_backup.json")
    }

    pub fn load() -> Self {
        std::fs::read_to_string(Self::path())
            .ok()
            .and_then(|t| serde_json::from_str(&t).ok())
            .unwrap_or_default()
    }

    pub fn save(&self) {
        if let Ok(t) = serde_json::to_string_pretty(self) {
            let _ = std::fs::write(Self::path(), t);
        }
    }
}

pub const TCP_INTERFACES: &str = r"SYSTEM\CurrentControlSet\Services\Tcpip\Parameters\Interfaces";

fn tcp_interfaces() -> Vec<String> {
    registry::subkeys(Hive::HKLM, TCP_INTERFACES)
}

fn reg_matches(cur: Option<RegValue>, want: &RegValue) -> bool {
    match (cur, want) {
        (Some(RegValue::Dword(a)), RegValue::Dword(b)) => a == *b,
        (Some(RegValue::Str(a)) | Some(RegValue::ExpandStr(a)), RegValue::Str(b)) => a.eq_ignore_ascii_case(b),
        _ => false,
    }
}

/// "A=1;B=0;" helpers for `Action::RegFlag`.
fn flag_get(list: &str, key: &str) -> Option<String> {
    list.split(';').filter_map(|e| e.split_once('=')).find(|(k, _)| k.trim().eq_ignore_ascii_case(key)).map(|(_, v)| v.trim().to_string())
}

fn flag_set(list: &str, key: &str, value: Option<&str>) -> String {
    let mut out: Vec<String> = list
        .split(';')
        .map(str::trim)
        .filter(|e| !e.is_empty() && !e.split_once('=').is_some_and(|(k, _)| k.trim().eq_ignore_ascii_case(key)))
        .map(str::to_string)
        .collect();
    if let Some(v) = value {
        out.push(format!("{key}={v}"));
    }
    out.iter().map(|e| format!("{e};")).collect()
}

fn action_state(a: &Action) -> Option<bool> {
    match a {
        Action::RegFlag { hive, path, name, key, value } => {
            Some(registry::get_string(*hive, path, name).and_then(|s| flag_get(&s, key)).as_deref() == Some(*value))
        }
        Action::Reg { hive, path, name, value, .. } => Some(reg_matches(registry::get(*hive, path, name), &value.reg())),
        Action::RegTcpInterfaces { name, value } => {
            let ifs = tcp_interfaces();
            if ifs.is_empty() {
                return None;
            }
            Some(ifs.iter().all(|i| {
                registry::get_dword(Hive::HKLM, &format!(r"{TCP_INTERFACES}\{i}"), name) == Some(*value)
            }))
        }
        Action::Service { name, start, .. } => services::query(name).map(|s| s.start_type == *start),
        Action::Task { path } => tasks::is_enabled(path).map(|enabled| !enabled),
    }
}

pub fn state(t: &Tweak) -> TweakState {
    let (mut total, mut ok) = (0, 0);
    for a in &t.actions {
        match action_state(a) {
            Some(true) => {
                total += 1;
                ok += 1;
            }
            Some(false) => total += 1,
            None => {}
        }
    }
    match (total, ok) {
        (0, _) => TweakState::NotApplicable,
        (t, o) if t == o => TweakState::Applied,
        (_, 0) => TweakState::NotApplied,
        _ => TweakState::Partial,
    }
}

pub fn apply(t: &Tweak) -> Result<(), String> {
    let mut backup = Backup::load();
    let mut saved = Vec::new();
    let mut errors = Vec::new();
    for a in &t.actions {
        match a {
            Action::Reg { hive, path, name, value, .. } => {
                saved.push(save_reg(*hive, path, name));
                if let Err(e) = registry::set(*hive, path, name, &value.reg()) {
                    errors.push(format!("{name}: {e}"));
                }
            }
            Action::RegFlag { hive, path, name, key, value } => {
                saved.push(save_reg(*hive, path, name));
                let cur = registry::get_string(*hive, path, name).unwrap_or_default();
                if let Err(e) = registry::set(*hive, path, name, &RegValue::Str(flag_set(&cur, key, Some(value)))) {
                    errors.push(format!("{name}: {e}"));
                }
            }
            Action::RegTcpInterfaces { name, value } => {
                for i in tcp_interfaces() {
                    let path = format!(r"{TCP_INTERFACES}\{i}");
                    saved.push(save_reg(Hive::HKLM, &path, name));
                    if let Err(e) = registry::set_dword(Hive::HKLM, &path, name, *value) {
                        errors.push(format!("{name}: {e}"));
                    }
                }
            }
            Action::Service { name, start, .. } => {
                if let Some(s) = services::query(name) {
                    saved.push(Saved::Service {
                        name: name.to_string(),
                        start: s.start_type,
                        delayed: s.delayed,
                        running: Some(s.running),
                    });
                    match services::set_start(name, *start, false) {
                        Ok(()) => {
                            if *start == services::START_DISABLED && s.running {
                                let _ = services::stop(name);
                            }
                        }
                        Err(e) => errors.push(format!("{name}: {e}")),
                    }
                }
            }
            Action::Task { path } => {
                if let Some(enabled) = tasks::is_enabled(path) {
                    saved.push(Saved::Task { path: path.to_string(), enabled });
                    if enabled {
                        if let Err(e) = tasks::set_enabled(path, false) {
                            errors.push(e);
                        }
                    }
                }
            }
        }
    }
    // Keep the *first* backup: it holds the true pre-Avix state.
    backup.entries.entry(t.id.to_string()).or_insert(saved);
    backup.save();
    if let Some(f) = t.live {
        f();
    }
    if errors.is_empty() {
        Ok(())
    } else {
        Err(errors.join("; "))
    }
}

fn restore_saved(s: &Saved, errors: &mut Vec<String>) {
    let r = match s {
        Saved::Reg { hive, path, name, value: Some(v), .. } => registry::set(*hive, path, name, v),
        Saved::Reg { hive, path, name, value: None, created } => {
            let r = registry::delete(*hive, path, name);
            // Remove keys that only existed because of this tweak (stops at the first non-empty one).
            for k in created {
                if !registry::delete_key_if_empty(*hive, k) {
                    break;
                }
            }
            r
        }
        Saved::Service { name, start, delayed, running } => {
            let r = services::set_start(name, *start, *delayed);
            // Restart only what was running before (old backups: auto-start services).
            let was_running = running.unwrap_or(*start == services::START_AUTO);
            if r.is_ok() && was_running && *start != services::START_DISABLED {
                let _ = services::start(name);
            }
            r
        }
        Saved::Task { path, enabled } => tasks::set_enabled(path, *enabled),
    };
    if let Err(e) = r {
        errors.push(e);
    }
}

pub fn revert(t: &Tweak) -> Result<(), String> {
    let mut backup = Backup::load();
    let mut errors = Vec::new();
    if let Some(saved) = backup.entries.remove(t.id) {
        for s in saved.iter().rev() {
            restore_saved(s, &mut errors);
        }
    } else {
        // Applied outside Avix (or backup lost): fall back to Windows defaults.
        for a in &t.actions {
            let r = match a {
                Action::Reg { hive, path, name, default: Some(v), .. } => registry::set(*hive, path, name, &v.reg()),
                Action::Reg { hive, path, name, default: None, .. } => registry::delete(*hive, path, name),
                Action::RegFlag { hive, path, name, key, .. } => {
                    let rest = flag_set(&registry::get_string(*hive, path, name).unwrap_or_default(), key, None);
                    if rest.is_empty() {
                        registry::delete(*hive, path, name)
                    } else {
                        registry::set(*hive, path, name, &RegValue::Str(rest))
                    }
                }
                Action::RegTcpInterfaces { name, .. } => {
                    for i in tcp_interfaces() {
                        let _ = registry::delete(Hive::HKLM, &format!(r"{TCP_INTERFACES}\{i}"), name);
                    }
                    Ok(())
                }
                Action::Service { name, default, .. } => {
                    if services::exists(name) {
                        services::set_start(name, *default, false)
                    } else {
                        Ok(())
                    }
                }
                Action::Task { path } => {
                    if tasks::is_enabled(path).is_some() {
                        tasks::set_enabled(path, true)
                    } else {
                        Ok(())
                    }
                }
            };
            if let Err(e) = r {
                errors.push(e);
            }
        }
    }
    backup.save();
    if let Some(f) = t.live {
        f();
    }
    if errors.is_empty() {
        Ok(())
    } else {
        Err(errors.join("; "))
    }
}

/// Create a System Restore point (temporarily lifting the 24 h creation throttle).
pub fn create_restore_point() -> Result<(), String> {
    let script = r#"$ErrorActionPreference = 'Stop'
$k = 'HKLM:\SOFTWARE\Microsoft\Windows NT\CurrentVersion\SystemRestore'
$old = (Get-ItemProperty -Path $k -Name SystemRestorePointCreationFrequency -ErrorAction SilentlyContinue).SystemRestorePointCreationFrequency
Set-ItemProperty -Path $k -Name SystemRestorePointCreationFrequency -Value 0 -Type DWord
try { Checkpoint-Computer -Description 'Avix - before tweaks' -RestorePointType MODIFY_SETTINGS }
finally {
  if ($null -eq $old) { Remove-ItemProperty -Path $k -Name SystemRestorePointCreationFrequency -ErrorAction SilentlyContinue }
  else { Set-ItemProperty -Path $k -Name SystemRestorePointCreationFrequency -Value $old -Type DWord }
}"#;
    crate::sys::powershell(script).map(|_| ()).map_err(|e| {
        if e.to_lowercase().contains("disabled") || e.contains("0x80070422") {
            "System Protection is off for your system drive (enable it in System Properties > System Protection)".into()
        } else {
            e.lines().next().unwrap_or("unknown error").to_string()
        }
    })
}

// ---------------------------------------------------------------- launch priority (IFEO)

const IFEO: &str = r"SOFTWARE\Microsoft\Windows NT\CurrentVersion\Image File Execution Options";

/// Windows applies this CPU priority when the exe starts - no handle to the game needed,
/// which is why it's the anti-cheat-friendly way to raise a protected game's priority.
pub fn launch_priority(exe: &str) -> Option<u32> {
    registry::get_dword(Hive::HKLM, &format!(r"{IFEO}\{exe}\PerfOptions"), "CpuPriorityClass")
}

/// 3 = High, 6 = Above normal, None = remove.
pub fn set_launch_priority(exe: &str, class: Option<u32>) -> Result<(), String> {
    let path = format!(r"{IFEO}\{exe}\PerfOptions");
    match class {
        Some(c) => registry::set_dword(Hive::HKLM, &path, "CpuPriorityClass", c),
        None => {
            registry::delete(Hive::HKLM, &path, "CpuPriorityClass")?;
            // Tidy up empty keys (never touches keys holding anything else).
            if registry::delete_key_if_empty(Hive::HKLM, &path) {
                registry::delete_key_if_empty(Hive::HKLM, &format!(r"{IFEO}\{exe}"));
            }
            Ok(())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{flag_get, flag_set};

    #[test]
    fn flag_lists() {
        assert_eq!(flag_set("", "SwapEffectUpgradeEnable", Some("1")), "SwapEffectUpgradeEnable=1;");
        let s = flag_set("VRROptimizeEnable=0;AutoHDREnable=1;", "SwapEffectUpgradeEnable", Some("1"));
        assert_eq!(s, "VRROptimizeEnable=0;AutoHDREnable=1;SwapEffectUpgradeEnable=1;");
        assert_eq!(flag_get(&s, "swapeffectupgradeenable").as_deref(), Some("1"));
        let s = flag_set("SwapEffectUpgradeEnable=0;AutoHDREnable=1;", "SwapEffectUpgradeEnable", Some("1"));
        assert_eq!(s, "AutoHDREnable=1;SwapEffectUpgradeEnable=1;");
        assert_eq!(flag_set(&s, "SwapEffectUpgradeEnable", None), "AutoHDREnable=1;");
        assert_eq!(flag_set("SwapEffectUpgradeEnable=1;", "SwapEffectUpgradeEnable", None), "");
    }
}
