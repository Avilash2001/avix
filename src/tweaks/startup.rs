//! Startup apps - enabled/disabled exactly the way Task Manager does it
//! (StartupApproved flags), so changes show up there too and are fully reversible.

use crate::sys::registry::{self, Hive, RegValue};
use std::path::PathBuf;

const APPROVED: &str = r"Software\Microsoft\Windows\CurrentVersion\Explorer\StartupApproved";

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Source {
    RunUser,
    RunMachine,
    RunMachine32,
    FolderUser,
    FolderMachine,
}

impl Source {
    pub fn label(self) -> &'static str {
        match self {
            Source::RunUser => "Registry (you)",
            Source::RunMachine => "Registry (all users)",
            Source::RunMachine32 => "Registry (all users, 32-bit)",
            Source::FolderUser => "Startup folder (you)",
            Source::FolderMachine => "Startup folder (all users)",
        }
    }

    fn approved(self) -> (Hive, String) {
        match self {
            Source::RunUser => (Hive::HKCU, format!(r"{APPROVED}\Run")),
            Source::RunMachine => (Hive::HKLM, format!(r"{APPROVED}\Run")),
            Source::RunMachine32 => (Hive::HKLM, format!(r"{APPROVED}\Run32")),
            Source::FolderUser => (Hive::HKCU, format!(r"{APPROVED}\StartupFolder")),
            Source::FolderMachine => (Hive::HKLM, format!(r"{APPROVED}\StartupFolder")),
        }
    }
}

#[derive(Clone, Debug)]
pub struct StartupItem {
    pub name: String,
    pub command: String,
    pub source: Source,
    pub enabled: bool,
}

fn approved_enabled(src: Source, name: &str) -> bool {
    let (hive, path) = src.approved();
    match registry::get(hive, &path, name) {
        Some(RegValue::Binary(b)) if !b.is_empty() => b[0] & 1 == 0,
        _ => true,
    }
}

fn folder(env: &str, rel: &str) -> Option<PathBuf> {
    std::env::var_os(env).map(|b| PathBuf::from(b).join(rel))
}

pub fn list() -> Vec<StartupItem> {
    let mut out = Vec::new();
    let runs = [
        (Hive::HKCU, r"Software\Microsoft\Windows\CurrentVersion\Run", Source::RunUser),
        (Hive::HKLM, r"SOFTWARE\Microsoft\Windows\CurrentVersion\Run", Source::RunMachine),
        (Hive::HKLM, r"SOFTWARE\WOW6432Node\Microsoft\Windows\CurrentVersion\Run", Source::RunMachine32),
    ];
    for (hive, path, src) in runs {
        for (name, v) in registry::values(hive, path) {
            let Some(cmd) = v.as_str() else { continue };
            if name.is_empty() {
                continue;
            }
            out.push(StartupItem { enabled: approved_enabled(src, &name), name, command: cmd.to_string(), source: src });
        }
    }
    let folders = [
        (folder("APPDATA", r"Microsoft\Windows\Start Menu\Programs\Startup"), Source::FolderUser),
        (folder("ProgramData", r"Microsoft\Windows\Start Menu\Programs\StartUp"), Source::FolderMachine),
    ];
    for (dir, src) in folders {
        let Some(dir) = dir else { continue };
        let Ok(rd) = std::fs::read_dir(&dir) else { continue };
        for e in rd.flatten() {
            let file = e.file_name().to_string_lossy().to_string();
            if file.eq_ignore_ascii_case("desktop.ini") {
                continue;
            }
            out.push(StartupItem {
                enabled: approved_enabled(src, &file),
                name: file,
                command: e.path().to_string_lossy().to_string(),
                source: src,
            });
        }
    }
    out.sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase()));
    out
}

pub fn set_enabled(item: &StartupItem, enabled: bool) -> Result<(), String> {
    let (hive, path) = item.source.approved();
    let mut data = vec![0u8; 12];
    if enabled {
        data[0] = 0x02;
    } else {
        data[0] = 0x03;
        let mut ft = windows_sys::Win32::Foundation::FILETIME { dwLowDateTime: 0, dwHighDateTime: 0 };
        unsafe { windows_sys::Win32::System::SystemInformation::GetSystemTimeAsFileTime(&mut ft) };
        let v = (ft.dwHighDateTime as u64) << 32 | ft.dwLowDateTime as u64;
        data[4..12].copy_from_slice(&v.to_le_bytes());
    }
    registry::set(hive, &path, &item.name, &RegValue::Binary(data))
}
