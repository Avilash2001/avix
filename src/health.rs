//! One-click health check for the usual causes of stutter, input lag and network lag.

use crate::sys::registry::{self, Hive};
use crate::sys::{self, display, memory, net, power, services, wide};
use crate::tweaks::{self, catalog, TweakState};

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum Severity {
    Bad,
    Warn,
    Info,
    Good,
}

#[derive(Clone, Debug)]
pub enum Fix {
    None,
    Tweak(&'static str),
    RefreshRate { device: String, hz: u32 },
    OpenStartup,
    OpenSettings,
}

#[derive(Clone, Debug)]
pub struct Finding {
    pub severity: Severity,
    pub title: String,
    pub detail: String,
    pub fix: Fix,
}

fn f(severity: Severity, title: impl Into<String>, detail: impl Into<String>, fix: Fix) -> Finding {
    Finding { severity, title: title.into(), detail: detail.into(), fix }
}

fn tweak_applied(id: &str) -> bool {
    catalog::find(id).is_some_and(|t| tweaks::state(t) == TweakState::Applied)
}

/// Hardware set-up problems that cost far more FPS than any Windows setting, and that Avix can't
/// fix itself (BIOS, RAM slots, drivers, power): it explains them instead.
fn hardware_findings(out: &mut Vec<Finding>) {
    use crate::sys::{games, hardware};

    // RAM: XMP/EXPO off, single channel, both sticks in one channel
    let sticks = hardware::memory_sticks();
    let slow: Vec<&hardware::MemStick> = sticks.iter().filter(|s| s.configured > 0 && s.rated >= s.configured + 200).collect();
    if let Some(s) = slow.first() {
        let desktop = s.is_desktop_dimm();
        out.push(f(
            if desktop { Severity::Bad } else { Severity::Info },
            format!("RAM runs at {} MT/s but is rated for {} MT/s", s.configured, s.rated),
            if desktop {
                format!(
                    "Your {} is running at its safe default speed. Turn on XMP (Intel boards) or EXPO / DOCP (AMD boards) in the BIOS — often 5–15% more FPS and better 1% lows, more with integrated graphics. Avix can't change BIOS settings.",
                    s.ddr_name()
                )
            } else {
                "Most laptops don't let you change this. If yours has a memory profile option in the BIOS, turning it on helps games noticeably.".to_string()
            },
            Fix::None,
        ));
    }
    let removable = sticks.iter().all(|s| matches!(s.form_factor, 0x09 | 0x0D));
    if sticks.len() == 1 && removable {
        out.push(f(
            Severity::Warn,
            format!("Only one RAM stick ({} GB) — single-channel memory", sticks[0].size_mb / 1024),
            "Two matching sticks run in dual-channel and double memory bandwidth: games (especially on integrated graphics) get noticeably smoother. Add a second identical stick.",
            Fix::None,
        ));
    } else if sticks.len() == 2 && removable {
        if let (Some(a), Some(b)) = (sticks[0].channel(), sticks[1].channel()) {
            if a == b {
                out.push(f(
                    Severity::Warn,
                    format!("Both RAM sticks are in channel {a}"),
                    "They run in single-channel. Move one stick so the pair sits in different channels (usually slots A2 and B2 — check your motherboard manual).",
                    Fix::None,
                ));
            }
        }
    }
    let total_gb: u64 = sticks.iter().map(|s| s.size_mb).sum::<u64>() / 1024;
    if (1..=8).contains(&total_gb) {
        out.push(f(
            Severity::Info,
            format!("{total_gb} GB of RAM"),
            "Tight for modern games. While boosting, Avix frees memory from background apps when RAM runs low — closing your browser before playing helps the most.",
            Fix::None,
        ));
    }

    // Graphics
    let gpus = hardware::gpus();
    if gpus.iter().any(|g| g.basic_driver) && !gpus.iter().any(|g| g.real) {
        out.push(f(
            Severity::Bad,
            "No graphics driver installed",
            "Windows is using the 'Microsoft Basic Display Adapter'. Install the driver from NVIDIA, AMD or Intel — games will run many times faster.",
            Fix::None,
        ));
    }
    let today = sys::local_date();
    for g in gpus.iter().filter(|g| g.real) {
        if let (Some((y, m, _)), Some((ty, tm, _))) = (g.driver_date, today) {
            let months = (ty * 12 + tm).saturating_sub(y * 12 + m);
            if months >= 12 {
                let site = if g.name.to_lowercase().contains("nvidia") {
                    "nvidia.com or the NVIDIA app"
                } else if g.name.to_lowercase().contains("intel") {
                    "intel.com (Driver & Support Assistant)"
                } else {
                    "amd.com or AMD Software: Adrenalin"
                };
                out.push(f(
                    Severity::Warn,
                    format!("{} driver is {} months old", g.name, months),
                    format!("New drivers often bring big fixes for recent games. Update from {site}."),
                    Fix::None,
                ));
            }
        }
    }
    if hardware::is_hybrid(&gpus) {
        let dgpu = gpus.iter().find(|g| g.real && !g.integrated).map(|g| g.name.clone()).unwrap_or_default();
        out.push(f(
            Severity::Info,
            format!("Two GPUs: games should use the {dgpu}"),
            "On laptops, games sometimes start on the weaker integrated GPU. Avix sets Windows' graphics preference to the dedicated GPU for every game it boosts (Settings > Games on the dedicated GPU).",
            Fix::None,
        ));
    }

    // Games on a hard disk
    let hdd: Vec<String> = games::game_drives()
        .into_iter()
        .filter(|d| hardware::is_hard_disk(*d) == Some(true))
        .map(|d| format!("{d}:"))
        .collect();
    if !hdd.is_empty() {
        out.push(f(
            Severity::Warn,
            format!("Games on {} are on a hard drive", hdd.join(", ")),
            "Hard drives make loading screens and texture streaming much slower and cause hitches in open-world games. Move the games you play most to an SSD (Steam: Settings > Storage > Move).",
            Fix::None,
        ));
    }

    // Laptops on battery
    if let Some(p) = hardware::power_source().filter(|p| p.has_battery) {
        if p.on_battery {
            out.push(f(
                Severity::Warn,
                "Running on battery",
                "Laptops cut CPU and GPU power on battery — often to half or less. Plug in to play.",
                Fix::None,
            ));
        }
        if p.battery_saver {
            out.push(f(
                Severity::Warn,
                "Battery saver is on",
                "It throttles the CPU and background work. Turn it off in Quick Settings while gaming.",
                Fix::None,
            ));
        }
    }
}

pub fn scan() -> Vec<Finding> {
    let mut out = Vec::new();

    if !sys::elevation::is_elevated() {
        out.push(f(
            Severity::Warn,
            "Avix is running without admin rights",
            "Pausing services, the standby cleaner, tweaks and app removal need administrator rights.",
            Fix::None,
        ));
    }

    // Displays
    for d in display::displays() {
        if d.max_hz > d.current_hz + 1 {
            out.push(f(
                Severity::Bad,
                format!("{} is running at {} Hz but supports {} Hz", d.monitor, d.current_hz, d.max_hz),
                format!(
                    "{}x{} on {}. You're throwing away smoothness and adding input lag — very common after driver updates or new cables.",
                    d.width, d.height, d.device
                ),
                Fix::RefreshRate { device: d.device.clone(), hz: d.max_hz },
            ));
        } else {
            out.push(f(
                Severity::Good,
                format!("{} runs at its max refresh rate ({} Hz)", d.monitor, d.current_hz),
                format!("{}x{}", d.width, d.height),
                Fix::None,
            ));
        }
    }

    // Game DVR / Game Mode / mouse
    if registry::get_dword(Hive::HKCU, r"System\GameConfigStore", "GameDVR_Enabled").unwrap_or(1) != 0 {
        out.push(f(
            Severity::Warn,
            "Game DVR background recording is on",
            "Xbox Game Bar keeps recording in the background, costing FPS and causing frame-time spikes.",
            Fix::Tweak("game_dvr_off"),
        ));
    }
    if registry::get_dword(Hive::HKCU, r"Software\Microsoft\GameBar", "AutoGameModeEnabled") == Some(0) {
        out.push(f(
            Severity::Warn,
            "Windows Game Mode is off",
            "Game Mode stops Windows Update from installing drivers or restarting while you play.",
            Fix::Tweak("game_mode_on"),
        ));
    }
    if registry::get_string(Hive::HKCU, r"Control Panel\Mouse", "MouseSpeed").as_deref().unwrap_or("1") != "0" {
        out.push(f(
            Severity::Info,
            "Mouse acceleration (Enhance pointer precision) is on",
            "Fine for desktop use, but makes muscle memory inconsistent outside raw-input games.",
            Fix::Tweak("mouse_accel_off"),
        ));
    }
    if registry::get_dword(
        Hive::HKLM,
        r"SYSTEM\CurrentControlSet\Control\DeviceGuard\Scenarios\HypervisorEnforcedCodeIntegrity",
        "Enabled",
    ) == Some(1)
    {
        out.push(f(
            Severity::Info,
            "Memory Integrity (HVCI) is on",
            "A real security feature that can cost ~5-10% in some CPU-bound games. Trade-off — only turn off if you understand the risk.",
            Fix::Tweak("hvci_off"),
        ));
    }

    hardware_findings(&mut out);

    // Power
    if let Some(g) = power::active() {
        let name = power::name_of(&g);
        if power::is_performance_plan(&g) {
            out.push(f(Severity::Good, format!("Power plan: {name}"), "Performance plan active.", Fix::None));
        } else {
            out.push(f(
                Severity::Info,
                format!("Power plan: {name}"),
                "Fine for the desktop — Avix switches to its tuned 'Avix Ultimate Performance' plan automatically while boosting.",
                Fix::None,
            ));
        }
    }

    // Memory
    let m = memory::status();
    let gb = |b: u64| b as f64 / 1024.0 / 1024.0 / 1024.0;
    if m.commit_limit.saturating_sub(m.total) < 512 * 1024 * 1024 {
        out.push(f(
            Severity::Bad,
            "No page file",
            "Without a page file, games and browsers crash instead of slowing down when RAM fills up. Let Windows manage the page file size.",
            Fix::None,
        ));
    }
    if m.load >= 85 {
        out.push(f(
            Severity::Warn,
            format!("Memory is {}% used right now", m.load),
            format!("{:.1} of {:.1} GB in use. Close heavy apps or let Avix throttle them while gaming.", gb(m.total - m.avail), gb(m.total)),
            Fix::None,
        ));
    }

    // Disk
    let sysdrive = std::env::var("SystemDrive").unwrap_or_else(|_| "C:".into()) + "\\";
    let (mut free, mut total, mut tf) = (0u64, 0u64, 0u64);
    let w = wide(&sysdrive);
    if unsafe { windows_sys::Win32::Storage::FileSystem::GetDiskFreeSpaceExW(w.as_ptr(), &mut free, &mut total, &mut tf) } != 0
        && total > 0
    {
        let pct = free as f64 / total as f64 * 100.0;
        if pct < 10.0 || free < 20 * 1024 * 1024 * 1024 {
            out.push(f(
                Severity::Warn,
                format!("System drive is almost full ({} free)", sys::fmt_bytes(free)),
                "Low free space slows updates, shader caches and the page file.",
                Fix::None,
            ));
        }
    }

    // Network
    let adapters = net::active_adapters();
    if adapters.iter().any(|a| a.wifi) && !adapters.iter().any(|a| !a.wifi) {
        out.push(f(
            Severity::Warn,
            "You're online over Wi-Fi",
            "Wi-Fi adds latency spikes and packet loss under interference — the #1 cause of 'network lag' in shooters. Use Ethernet if you can.",
            Fix::None,
        ));
    }
    for a in adapters.iter().filter(|a| !a.wifi && a.speed_mbps > 0 && a.speed_mbps < 1000) {
        out.push(f(
            Severity::Info,
            format!("{} is linked at {} Mbps", a.name, a.speed_mbps),
            "A gigabit port running at 100 Mbps usually means a damaged/old cable (use Cat5e or better).",
            Fix::None,
        ));
    }
    if !tweak_applied("delivery_optimization_p2p_off") {
        out.push(f(
            Severity::Info,
            "Windows may upload updates to other PCs",
            "Delivery Optimization peer-to-peer can use your upload bandwidth.",
            Fix::Tweak("delivery_optimization_p2p_off"),
        ));
    }

    // Background noise
    if services::is_running("DiagTrack") {
        out.push(f(
            Severity::Info,
            "Telemetry service is running",
            "Connected User Experiences & Telemetry collects and uploads usage data in the background.",
            Fix::Tweak("telemetry_services"),
        ));
    }
    if registry::key_exists(Hive::HKLM, r"SOFTWARE\Microsoft\Windows\CurrentVersion\WindowsUpdate\Auto Update\RebootRequired") {
        out.push(f(
            Severity::Warn,
            "A Windows Update restart is pending",
            "Servicing work keeps running in the background until you restart.",
            Fix::None,
        ));
    }
    let enabled_startup = tweaks::startup::list().iter().filter(|s| s.enabled).count();
    if enabled_startup > 8 {
        out.push(f(
            Severity::Info,
            format!("{enabled_startup} apps start with Windows"),
            "Each one costs boot time and usually keeps running in the background.",
            Fix::OpenStartup,
        ));
    }
    if !crate::autostart::is_enabled() {
        out.push(f(
            Severity::Info,
            "Avix doesn't start with Windows",
            "Turn it on so boosts happen automatically every time you play.",
            Fix::OpenSettings,
        ));
    }

    out.sort_by_key(|x| x.severity);
    out
}
