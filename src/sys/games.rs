//! Games Windows itself has recognised - the list Xbox Game Bar and Game Mode use.
//! Every PC game Windows has seen launch gets an entry under
//! HKCU\System\GameConfigStore\Children\<id> with the exe's full path.

use super::registry::{self, Hive};

const STORE: &str = r"System\GameConfigStore\Children";

/// Generic hosts (e.g. Minecraft Java runs as javaw.exe): only ever matched by full path.
const GENERIC: &[&str] = &[
    "java.exe", "javaw.exe", "python.exe", "pythonw.exe", "node.exe", "dotnet.exe", "cmd.exe",
    "powershell.exe", "pwsh.exe", "rundll32.exe", "dllhost.exe", "electron.exe",
];

#[derive(Clone, Debug, PartialEq)]
pub struct KnownGame {
    /// File name as Windows stored it (e.g. "F1_25.exe"), for display.
    pub name: String,
    /// Full exe path, lowercase.
    pub path_lc: String,
    /// File name, lowercase (e.g. "f1_25.exe").
    pub name_lc: String,
}

impl KnownGame {
    /// The file name alone is distinctive enough to trust when the full path can't be read.
    pub fn name_is_distinctive(&self) -> bool {
        !GENERIC.contains(&self.name_lc.as_str())
    }
}

/// Every game exe Windows knows about for the current user, sorted by file name.
pub fn windows_known() -> Vec<KnownGame> {
    let mut out: Vec<KnownGame> = Vec::new();
    for id in registry::subkeys(Hive::HKCU, STORE) {
        let Some(path) = registry::get_string(Hive::HKCU, &format!(r"{STORE}\{id}"), "MatchedExeFullPath") else {
            continue;
        };
        let path = path.trim().trim_matches('"');
        let name = path.rsplit(['\\', '/']).next().unwrap_or_default().to_string();
        let name_lc = name.to_lowercase();
        if !name_lc.ends_with(".exe") {
            continue;
        }
        let path_lc = path.to_lowercase();
        if !out.iter().any(|g| g.path_lc == path_lc) {
            out.push(KnownGame { name, path_lc, name_lc });
        }
    }
    out.sort_by(|a, b| a.name_lc.cmp(&b.name_lc));
    out
}
