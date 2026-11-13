//! User configuration: boost profiles + global safety lists.
//! Stored as JSON at %LOCALAPPDATA%\Avix\config.json.

use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum PowerPlan {
    Keep,
    HighPerformance,
    #[default]
    AvixUltimate,
}

impl PowerPlan {
    pub const ALL: [PowerPlan; 3] = [PowerPlan::AvixUltimate, PowerPlan::HighPerformance, PowerPlan::Keep];
    pub fn label(self) -> &'static str {
        match self {
            PowerPlan::Keep => "Don't change",
            PowerPlan::HighPerformance => "High Performance",
            PowerPlan::AvixUltimate => "Avix Ultimate (tuned)",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum TargetPriority {
    Keep,
    AboveNormal,
    #[default]
    High,
}

impl TargetPriority {
    pub const ALL: [TargetPriority; 3] = [TargetPriority::High, TargetPriority::AboveNormal, TargetPriority::Keep];
    pub fn label(self) -> &'static str {
        match self {
            TargetPriority::Keep => "Don't change",
            TargetPriority::AboveNormal => "Above normal",
            TargetPriority::High => "High",
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct Profile {
    pub name: String,
    pub enabled: bool,
    /// Executable names (lowercase) that activate this profile.
    pub triggers: Vec<String>,
    /// Only activate while a trigger uses at least this much CPU (% of one core). 0 = always.
    pub min_cpu_core_percent: f32,
    pub power_plan: PowerPlan,
    pub priority: TargetPriority,
    /// Lower priority of everything else in your session while boosting.
    pub throttle_background: bool,
    /// Put throttled apps in EcoQoS ("Efficiency mode").
    pub efficiency_mode: bool,
    /// Give throttled apps low disk I/O priority.
    pub lower_io_priority: bool,
    /// Apps to keep snappy (Above normal) alongside the target, e.g. OBS.
    pub companions: Vec<String>,
    /// Services to pause while boosting (restarted afterwards).
    pub pause_services: Vec<String>,
    /// Apps to close when the boost starts (not restarted).
    pub close_apps: Vec<String>,
    /// Request 0.5 ms system timer resolution while boosting.
    pub timer_resolution: bool,
    /// Purge the standby list when free memory runs low.
    pub standby_cleaner: bool,
}

impl Default for Profile {
    fn default() -> Self {
        Self {
            name: "New profile".into(),
            enabled: true,
            triggers: Vec::new(),
            min_cpu_core_percent: 0.0,
            power_plan: PowerPlan::AvixUltimate,
            priority: TargetPriority::High,
            throttle_background: true,
            efficiency_mode: true,
            lower_io_priority: true,
            companions: Vec::new(),
            pause_services: Vec::new(),
            close_apps: Vec::new(),
            timer_resolution: false,
            standby_cleaner: false,
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct Config {
    /// Boost any game Windows has recognised (Game Bar's list), even if it isn't in a profile.
    pub windows_games: bool,
    /// Boost any app that goes fullscreen (games not in a profile yet).
    pub auto_fullscreen: bool,
    pub fullscreen_profile: String,
    pub fullscreen_ignore: Vec<String>,
    /// Never modified by Avix, ever.
    pub never_touch: Vec<String>,
    /// Games with kernel anti-cheat: Avix never opens a handle to them.
    pub anticheat_protected: Vec<String>,
    /// Service-session processes that are still worth throttling while boosting.
    pub background_hogs: Vec<String>,
    /// Ctrl+Alt+B toggles "Boost now".
    pub hotkey: bool,
    pub notifications: bool,
    pub poll_interval_ms: u64,
    pub standby_min_mb: u64,
    pub free_memory_below_mb: u64,
    pub ping_host: String,
    pub profiles: Vec<Profile>,
}

fn s(list: &[&str]) -> Vec<String> {
    list.iter().map(|x| x.to_lowercase()).collect()
}

pub const SUGGESTED_SERVICES: &[(&str, &str)] = &[
    ("wuauserv", "Windows Update"),
    ("UsoSvc", "Update Orchestrator"),
    ("DoSvc", "Delivery Optimization (update P2P)"),
    ("BITS", "Background Intelligent Transfer"),
    ("WSearch", "Windows Search indexer"),
    ("SysMain", "SysMain / Superfetch"),
    ("Spooler", "Print Spooler"),
    ("DiagTrack", "Telemetry"),
];

pub const SUGGESTED_CLOSE_APPS: &[&str] = &[
    "onedrive.exe",
    "teams.exe",
    "ms-teams.exe",
    "msedgewebview2.exe",
    "phoneexperiencehost.exe",
    "widgets.exe",
    "steamwebhelper.exe",
    "epicgameslauncher.exe",
    "spotify.exe",
];

impl Default for Config {
    fn default() -> Self {
        Self {
            windows_games: true,
            auto_fullscreen: true,
            fullscreen_profile: "Gaming".into(),
            fullscreen_ignore: s(&[
                "chrome.exe", "msedge.exe", "firefox.exe", "brave.exe", "opera.exe", "opera_gx.exe",
                "vivaldi.exe", "vlc.exe", "mpv.exe", "potplayermini64.exe", "mpc-hc64.exe",
                "microsoft.media.player.exe", "applicationframehost.exe", "powerpnt.exe",
                "explorer.exe", "searchhost.exe", "startmenuexperiencehost.exe", "lockapp.exe",
                "shellexperiencehost.exe", "avix.exe", "netflix.exe", "code.exe",
            ]),
            never_touch: s(&[
                // Windows core
                "system", "registry", "idle", "memory compression", "secure system", "smss.exe",
                "csrss.exe", "wininit.exe", "winlogon.exe", "services.exe", "lsass.exe", "lsaiso.exe",
                "svchost.exe", "dwm.exe", "fontdrvhost.exe", "audiodg.exe", "explorer.exe",
                "sihost.exe", "ctfmon.exe", "conhost.exe", "taskmgr.exe", "textinputhost.exe",
                "msmpeng.exe", "nissrv.exe", "mpdefendercoreservice.exe", "securityhealthservice.exe",
                "securityhealthsystray.exe", "smartscreen.exe", "wudfhost.exe", "avix.exe",
                // anti-cheat & game launchers
                "vgc.exe", "vgtray.exe", "riotclientservices.exe", "riotclientux.exe",
                "riotclientuxrender.exe", "riotclientcrashhandler.exe", "easyanticheat.exe",
                "easyanticheat_eos.exe", "eaanticheat.gameservice.exe", "beservice.exe",
                "beservice_x64.exe", "faceit.exe", "faceitservice.exe", "faceitclient.exe",
                // voice & audio
                "discord.exe", "discordptb.exe", "discordcanary.exe", "ts3client_win64.exe",
                "teamspeak.exe", "voicemeeter.exe", "voicemeeterpro.exe", "voicemeeter8.exe",
                "voicemeeter8x64.exe",
                // peripherals (input latency!)
                "lghub.exe", "lghub_agent.exe", "lghub_system_tray.exe", "logioptionsplus_agent.exe",
                "razer synapse 3.exe", "razer synapse service process.exe", "razercentralservice.exe",
                "icue.exe", "steelseriesgg.exe", "steelseriesengine.exe", "wootility.exe",
                // GPU drivers & capture
                "nvcontainer.exe", "nvdisplay.container.exe", "nvidia overlay.exe", "nvidia share.exe",
                "nvidia app.exe", "amdrsserv.exe", "radeonsoftware.exe", "atieclxx.exe", "amdow.exe",
            ]),
            anticheat_protected: s(&[
                "valorant-win64-shipping.exe", "valorant.exe", "league of legends.exe",
                "fortniteclient-win64-shipping.exe", "r5apex.exe", "r5apex_dx12.exe", "cod.exe",
                "rainbowsix.exe", "rainbowsix_be.exe", "tslgame.exe", "destiny2.exe", "eldenring.exe",
                "escapefromtarkov.exe", "rustclient.exe", "dayz_x64.exe", "gta5.exe",
                "gta5_enhanced.exe", "marvel-win64-shipping.exe", "overwatch.exe", "bf2042.exe",
                "discovery.exe", "fc25.exe", "fc26.exe",
            ]),
            background_hogs: s(&[
                "searchindexer.exe", "searchprotocolhost.exe", "searchfilterhost.exe",
                "compattelrunner.exe", "mousocoreworker.exe", "tiworker.exe", "wsappx.exe",
                "msiexec.exe", "dismhost.exe", "onedrive.exe",
            ]),
            hotkey: true,
            notifications: false,
            poll_interval_ms: 2000,
            standby_min_mb: 1024,
            free_memory_below_mb: 2048,
            ping_host: "1.1.1.1".into(),
            profiles: vec![
                Profile {
                    name: "Gaming".into(),
                    triggers: s(&[
                        "VALORANT-Win64-Shipping.exe", "cs2.exe", "FortniteClient-Win64-Shipping.exe",
                        "r5apex.exe", "r5apex_dx12.exe", "League of Legends.exe", "Overwatch.exe",
                        "RainbowSix.exe", "cod.exe", "GTA5.exe", "GTA5_Enhanced.exe", "RocketLeague.exe",
                        "dota2.exe", "Marvel-Win64-Shipping.exe", "eldenring.exe", "destiny2.exe",
                        "TslGame.exe", "EscapeFromTarkov.exe", "RustClient.exe", "Cyberpunk2077.exe",
                        "bg3.exe", "bg3_dx11.exe", "helldivers2.exe", "deadlock.exe",
                        "Minecraft.Windows.exe", "Discovery.exe",
                    ]),
                    min_cpu_core_percent: 0.0,
                    power_plan: PowerPlan::AvixUltimate,
                    priority: TargetPriority::High,
                    throttle_background: true,
                    efficiency_mode: true,
                    lower_io_priority: true,
                    companions: s(&["obs64.exe", "obs32.exe", "streamlabs obs.exe", "streamlabs desktop.exe"]),
                    pause_services: vec!["wuauserv".into(), "UsoSvc".into(), "DoSvc".into(), "BITS".into()],
                    close_apps: Vec::new(),
                    timer_resolution: false,
                    standby_cleaner: true,
                    enabled: true,
                },
                Profile {
                    name: "Streaming".into(),
                    triggers: s(&["obs64.exe", "obs32.exe", "streamlabs obs.exe", "streamlabs desktop.exe", "xsplit.core.exe"]),
                    min_cpu_core_percent: 0.0,
                    power_plan: PowerPlan::AvixUltimate,
                    priority: TargetPriority::AboveNormal,
                    throttle_background: true,
                    efficiency_mode: true,
                    lower_io_priority: true,
                    companions: Vec::new(),
                    pause_services: vec!["wuauserv".into(), "DoSvc".into(), "BITS".into()],
                    close_apps: Vec::new(),
                    timer_resolution: false,
                    standby_cleaner: false,
                    enabled: true,
                },
                Profile {
                    name: "AI / Compute".into(),
                    triggers: s(&[
                        "ollama.exe", "ollama_llama_server.exe", "llama-server.exe", "llama-cli.exe",
                        "lm studio.exe", "lms.exe", "koboldcpp.exe", "jan.exe", "gpt4all.exe",
                        "comfyui.exe", "blender.exe",
                    ]),
                    min_cpu_core_percent: 20.0,
                    power_plan: PowerPlan::AvixUltimate,
                    priority: TargetPriority::AboveNormal,
                    throttle_background: true,
                    efficiency_mode: true,
                    lower_io_priority: false,
                    companions: Vec::new(),
                    pause_services: vec!["wuauserv".into(), "DoSvc".into()],
                    close_apps: Vec::new(),
                    timer_resolution: false,
                    standby_cleaner: false,
                    enabled: true,
                },
            ],
        }
    }
}

impl Config {
    pub fn path() -> PathBuf {
        crate::sys::data_dir().join("config.json")
    }

    pub fn load() -> Self {
        let mut c: Config = match std::fs::read_to_string(Self::path()) {
            Ok(t) => {
                // Notepad & PowerShell 5 like to add a UTF-8 BOM.
                let t = t.trim_start_matches('\u{feff}');
                match serde_json::from_str(t) {
                    Ok(c) => c,
                    Err(e) => {
                        crate::logging::log(&format!("config.json is invalid ({e}) - using defaults until it's fixed"));
                        Config::default()
                    }
                }
            }
            Err(_) => Config::default(),
        };
        c.normalize();
        if !Self::path().exists() {
            let _ = c.save();
        }
        c
    }

    pub fn save(&self) -> Result<(), String> {
        let text = serde_json::to_string_pretty(self).map_err(|e| e.to_string())?;
        let tmp = Self::path().with_extension("json.tmp");
        std::fs::write(&tmp, text).map_err(|e| e.to_string())?;
        std::fs::rename(&tmp, Self::path()).map_err(|e| e.to_string())
    }

    /// Lowercase + dedupe every exe list so matching is a plain string compare.
    pub fn normalize(&mut self) {
        fn norm(v: &mut Vec<String>) {
            for x in v.iter_mut() {
                *x = x.trim().to_lowercase();
            }
            v.retain(|x| !x.is_empty());
            let mut seen = std::collections::HashSet::new();
            v.retain(|x| seen.insert(x.clone()));
        }
        norm(&mut self.fullscreen_ignore);
        norm(&mut self.never_touch);
        norm(&mut self.anticheat_protected);
        norm(&mut self.background_hogs);
        for p in &mut self.profiles {
            norm(&mut p.triggers);
            norm(&mut p.companions);
            norm(&mut p.close_apps);
            p.pause_services.retain(|x| !x.trim().is_empty());
        }
        self.poll_interval_ms = self.poll_interval_ms.clamp(500, 10_000);
    }

    pub fn profile(&self, name: &str) -> Option<&Profile> {
        self.profiles.iter().find(|p| p.name.eq_ignore_ascii_case(name))
    }
}

pub fn contains(list: &[String], name_lc: &str) -> bool {
    list.iter().any(|x| x == name_lc)
}
