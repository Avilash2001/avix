//! Working out what's a game, on any PC, without a hard-coded list:
//! - Windows' own game list (what Xbox Game Bar and Game Mode use),
//! - the install folders of game stores (Steam, Epic, GOG, Ubisoft, EA, Xbox, itch.io, Amazon),
//! - plus a list of launchers / overlays / tools that live next to games but aren't games.

use super::registry::{self, Hive};

const STORE: &str = r"System\GameConfigStore\Children";

/// Generic hosts (e.g. Minecraft Java runs as javaw.exe): only ever matched by full path.
const GENERIC: &[&str] = &[
    "java.exe", "javaw.exe", "python.exe", "pythonw.exe", "node.exe", "dotnet.exe", "cmd.exe",
    "powershell.exe", "pwsh.exe", "rundll32.exe", "dllhost.exe", "electron.exe",
];

/// Launchers, store clients, overlays, recorders, scalers, wallpaper engines, crash handlers and
/// installers: they sit in game folders or cover the screen, but boosting them makes no sense.
const NOT_GAMES: &[&str] = &[
    // launchers / store clients
    "steam.exe", "steamwebhelper.exe", "steamservice.exe", "steamerrorreporter.exe", "steamerrorreporter64.exe",
    "epicgameslauncher.exe", "epicwebhelper.exe", "epiconlineservicesuserhelper.exe", "epiconlineservicesuihelper.exe", "galaxyclient.exe",
    "galaxyclient helper.exe", "gog galaxy notifications renderer.exe", "eadesktop.exe", "eabackgroundservice.exe",
    "ealauncher.exe", "origin.exe", "battle.net.exe", "agent.exe", "upc.exe", "ubisoftconnect.exe",
    "uplaywebcore.exe", "riotclientservices.exe", "riotclientux.exe", "riotclientuxrender.exe", "riot client.exe",
    "leagueclient.exe", "leagueclientux.exe", "leagueclientuxrender.exe", "launcher.exe", "launcherpatcher.exe",
    "rockstarlauncher.exe", "socialclubhelper.exe", "playnite.desktopapp.exe", "playnite.fullscreenapp.exe",
    "heroic.exe", "itch.exe", "amazon games ui.exe", "xboxpcapp.exe", "xboxapp.exe", "minecraftlauncher.exe",
    "modrinth app.exe", "curseforge.exe", "prismlauncher.exe", "start_protected_game.exe",
    "easyanticheat.exe", "easyanticheat_eos.exe", "easyanticheat_eos_setup.exe", "beservice.exe",
    // overlays, recorders, monitoring, scalers
    "gameoverlayui.exe", "gameoverlayui64.exe", "gamebar.exe", "gamebarftserver.exe", "xboxgamebarwidgets.exe",
    "nvidia overlay.exe", "nvidia share.exe", "nvidia app.exe", "nvcontainer.exe", "radeonsoftware.exe",
    "amdrsserv.exe", "amdow.exe", "discord.exe", "overwolf.exe", "overwolfbrowser.exe", "medal.exe",
    "rtss.exe", "msiafterburner.exe", "losslessscaling.exe", "magpie.exe", "fpsmonitor.exe",
    "screenclippinghost.exe", "snippingtool.exe", "sharex.exe", "obs64.exe", "obs32.exe",
    // desktop customisation
    "wallpaper32.exe", "wallpaper64.exe", "webwallpaper32.exe", "ui32.exe", "lively.exe", "rainmeter.exe",
    "displayfusion.exe",
    // VR runtime
    "vrserver.exe", "vrmonitor.exe", "vrcompositor.exe", "vrdashboard.exe", "vrwebhelper.exe", "vrstartup.exe",
    // remote desktops / VMs
    "mstsc.exe", "msrdc.exe", "vmconnect.exe", "vmware.exe", "virtualboxvm.exe",
    // crash handlers, helpers
    "crashreportclient.exe", "unrealcefsubprocess.exe", "unitycrashhandler64.exe", "unitycrashhandler32.exe",
    "crashpad_handler.exe", "cefsharp.browsersubprocess.exe", "qtwebengineprocess.exe",
];

/// Installers and redistributables that run from game folders on first launch.
const INSTALLER_PREFIXES: &[&str] = &[
    "vc_redist", "vcredist", "dxsetup", "dotnet", "ndp", "oalinst", "physx", "ue4prereq", "ueprereq",
    "unins", "uninstall", "setup", "installer",
];

/// Anything here (or a screen saver) is never treated as a game.
pub fn is_not_game(name_lc: &str) -> bool {
    NOT_GAMES.contains(&name_lc)
        || name_lc.ends_with(".scr")
        || INSTALLER_PREFIXES.iter().any(|p| name_lc.starts_with(p))
}

// ------------------------------------------------------------------ Windows' game list

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

// ------------------------------------------------------------------ store libraries

/// Folder names game stores install into - these cover every Steam library, the default
/// Epic / EA / GOG / Ubisoft / Xbox / itch.io / Amazon locations and "D:\Games" style folders.
const LIBRARY_PATTERNS: &[(&str, &str)] = &[
    (r"\steamapps\common\", "Steam"),
    (r"\epic games\", "Epic Games"),
    (r"\gog galaxy\games\", "GOG"),
    (r"\gog games\", "GOG"),
    (r"\ubisoft game launcher\games\", "Ubisoft"),
    (r"\ea games\", "EA"),
    (r"\xboxgames\", "Xbox"),
    (r"\itch\apps\", "itch.io"),
    (r"\amazon games\library\", "Amazon Games"),
    (r"\games\", "a Games folder"),
];

/// A custom install location read from a store's own records (lowercase, ends with '\').
#[derive(Clone, Debug, PartialEq)]
pub struct LibraryRoot {
    pub root: String,
    pub store: &'static str,
}

fn push_root(out: &mut Vec<LibraryRoot>, path: &str, store: &'static str) {
    let mut root = path.trim().trim_matches('"').replace('/', "\\").to_lowercase();
    if root.len() < 4 {
        return; // never a whole drive
    }
    if !root.ends_with('\\') {
        root.push('\\');
    }
    if !out.iter().any(|r| r.root == root) {
        out.push(LibraryRoot { root, store });
    }
}

/// Games installed to custom folders, from Epic's manifests and GOG / Ubisoft's registry records.
pub fn library_roots() -> Vec<LibraryRoot> {
    let mut out = Vec::new();
    let pd = std::env::var("ProgramData").unwrap_or_else(|_| r"C:\ProgramData".into());
    if let Ok(dir) = std::fs::read_dir(format!(r"{pd}\Epic\EpicGamesLauncher\Data\Manifests")) {
        for e in dir.flatten() {
            if e.path().extension().is_some_and(|x| x.eq_ignore_ascii_case("item")) {
                let loc = std::fs::read_to_string(e.path())
                    .ok()
                    .and_then(|t| serde_json::from_str::<serde_json::Value>(&t).ok())
                    .and_then(|v| v.get("InstallLocation").and_then(|x| x.as_str()).map(str::to_string));
                if let Some(loc) = loc {
                    push_root(&mut out, &loc, "Epic Games");
                }
            }
        }
    }
    for (key, value, store) in [
        (r"SOFTWARE\WOW6432Node\GOG.com\Games", "path", "GOG"),
        (r"SOFTWARE\WOW6432Node\Ubisoft\Launcher\Installs", "InstallDir", "Ubisoft"),
    ] {
        for id in registry::subkeys(Hive::HKLM, key) {
            if let Some(p) = registry::get_string(Hive::HKLM, &format!(r"{key}\{id}"), value) {
                push_root(&mut out, &p, store);
            }
        }
    }
    out
}

/// Every Steam library's `steamapps\common` folder (from Steam's own libraryfolders.vdf).
pub fn steam_libraries() -> Vec<String> {
    let Some(steam) = registry::get_string(Hive::HKCU, r"Software\Valve\Steam", "SteamPath") else {
        return Vec::new();
    };
    let steam = steam.replace('/', "\\");
    let mut out = vec![format!(r"{steam}\steamapps\common")];
    if let Ok(vdf) = std::fs::read_to_string(format!(r"{steam}\steamapps\libraryfolders.vdf")) {
        for line in vdf.lines() {
            let l = line.trim();
            if let Some(rest) = l.strip_prefix("\"path\"") {
                let p = rest.trim().trim_matches('"').replace(r"\\", r"\");
                let lib = format!(r"{p}\steamapps\common");
                if !out.iter().any(|x| x.eq_ignore_ascii_case(&lib)) {
                    out.push(lib);
                }
            }
        }
    }
    out
}

/// Drive letters that hold games: Windows' game list, Steam libraries and other stores' installs.
pub fn game_drives() -> Vec<char> {
    let mut paths: Vec<String> = windows_known().into_iter().map(|g| g.path_lc).collect();
    paths.extend(steam_libraries().into_iter().filter(|p| std::path::Path::new(p).is_dir()));
    paths.extend(library_roots().into_iter().map(|r| r.root));
    let mut drives: Vec<char> = paths
        .iter()
        .filter_map(|p| {
            let mut c = p.chars();
            match (c.next(), c.next()) {
                (Some(d), Some(':')) if d.is_ascii_alphabetic() => Some(d.to_ascii_uppercase()),
                _ => None,
            }
        })
        .collect();
    drives.sort_unstable();
    drives.dedup();
    drives
}

/// Which store installed this exe, if any.
pub fn library_of(path_lc: &str, roots: &[LibraryRoot]) -> Option<&'static str> {
    roots
        .iter()
        .find(|r| path_lc.starts_with(&r.root))
        .map(|r| r.store)
        .or_else(|| LIBRARY_PATTERNS.iter().find(|(p, _)| path_lc.contains(p)).map(|(_, s)| *s))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn libraries() {
        let roots = vec![LibraryRoot { root: r"e:\my stuff\fortnite\".into(), store: "Epic Games" }];
        let f = |p: &str| library_of(&p.to_lowercase(), &roots);
        assert_eq!(f(r"D:\SteamLibrary\steamapps\common\One More Lawn\OneMoreLawn.exe"), Some("Steam"));
        assert_eq!(f(r"C:\Program Files (x86)\Steam\steamapps\common\F1 25\F1_25.exe"), Some("Steam"));
        assert_eq!(f(r"E:\My Stuff\Fortnite\FortniteGame\Binaries\Win64\x.exe"), Some("Epic Games"));
        assert_eq!(f(r"C:\XboxGames\Forza\Content\forza.exe"), Some("Xbox"));
        assert_eq!(f(r"D:\Games\Skate\Skate.exe"), Some("a Games folder"));
        assert_eq!(f(r"C:\Users\x\Documents\My Games\thing.exe"), None);
        assert_eq!(f(r"C:\Program Files\Mozilla Firefox\firefox.exe"), None);
        assert!(is_not_game("riotclientservices.exe"));
        assert!(is_not_game("vc_redist.x64.exe"));
        assert!(is_not_game("bubbles.scr"));
        assert!(!is_not_game("onemorelawn.exe"));
    }
}
