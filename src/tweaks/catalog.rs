//! The tweak catalogue. Descriptions are deliberately honest about what each
//! setting does (and doesn't) do.

use super::{Action, Category, Restart, Risk, Tweak, V};
use crate::sys::registry::Hive::{HKCU, HKLM};
use crate::sys::services::{START_AUTO, START_DISABLED, START_MANUAL};
use std::sync::OnceLock;

fn lm(path: &'static str, name: &'static str, value: V, default: Option<V>) -> Action {
    Action::Reg { hive: HKLM, path, name, value, default }
}

fn cu(path: &'static str, name: &'static str, value: V, default: Option<V>) -> Action {
    Action::Reg { hive: HKCU, path, name, value, default }
}

fn svc(name: &'static str, start: u32, default: u32) -> Action {
    Action::Service { name, start, default }
}

fn task(path: &'static str) -> Action {
    Action::Task { path }
}

use V::{D, S};

const MM: &str = r"SOFTWARE\Microsoft\Windows NT\CurrentVersion\Multimedia\SystemProfile";
const MM_GAMES: &str = r"SOFTWARE\Microsoft\Windows NT\CurrentVersion\Multimedia\SystemProfile\Tasks\Games";
const CDM: &str = r"Software\Microsoft\Windows\CurrentVersion\ContentDeliveryManager";
const ADV: &str = r"Software\Microsoft\Windows\CurrentVersion\Explorer\Advanced";
const DATACOLLECTION: &str = r"SOFTWARE\Policies\Microsoft\Windows\DataCollection";

fn mouse_live() {
    use crate::sys::registry::{self, Hive};
    use windows_sys::Win32::UI::WindowsAndMessaging::{SystemParametersInfoW, SPIF_SENDCHANGE, SPI_SETMOUSE};
    let get = |n: &str| {
        registry::get_string(Hive::HKCU, r"Control Panel\Mouse", n).and_then(|s| s.trim().parse::<i32>().ok()).unwrap_or(0)
    };
    let mut v = [get("MouseThreshold1"), get("MouseThreshold2"), get("MouseSpeed")];
    unsafe {
        SystemParametersInfoW(SPI_SETMOUSE, 0, v.as_mut_ptr() as *mut std::ffi::c_void, SPIF_SENDCHANGE);
    }
}

pub fn all() -> &'static [Tweak] {
    static CATALOG: OnceLock<Vec<Tweak>> = OnceLock::new();
    CATALOG.get_or_init(build)
}

pub fn find(id: &str) -> Option<&'static Tweak> {
    all().iter().find(|t| t.id == id)
}

#[allow(clippy::too_many_lines)]
fn build() -> Vec<Tweak> {
    vec![
        // ------------------------------------------------------------ Gaming
        Tweak {
            id: "game_dvr_off",
            category: Category::Gaming,
            title: "Turn off Game DVR background recording",
            description: "Xbox Game Bar's background capture keeps recording your gameplay to RAM, costing FPS and adding frame-time spikes. Screenshots via Win+Alt+PrtScn still work.",
            risk: Risk::Low,
            recommended: true,
            restart: Restart::None,
            actions: vec![
                cu(r"System\GameConfigStore", "GameDVR_Enabled", D(0), Some(D(1))),
                cu(r"Software\Microsoft\Windows\CurrentVersion\GameDVR", "AppCaptureEnabled", D(0), None),
                lm(r"SOFTWARE\Policies\Microsoft\Windows\GameDVR", "AllowGameDVR", D(0), None),
            ],
            live: None,
        },
        Tweak {
            id: "game_mode_on",
            category: Category::Gaming,
            title: "Make sure Windows Game Mode is ON",
            description: "On Windows 11, Game Mode stops Windows Update from installing drivers / rebooting mid-game and prioritises the game. It works together with Avix.",
            risk: Risk::Low,
            recommended: true,
            restart: Restart::None,
            actions: vec![
                cu(r"Software\Microsoft\GameBar", "AutoGameModeEnabled", D(1), None),
                cu(r"Software\Microsoft\GameBar", "AllowAutoGameMode", D(1), None),
            ],
            live: None,
        },
        Tweak {
            id: "mouse_accel_off",
            category: Category::Gaming,
            title: "Disable mouse acceleration (Enhance pointer precision)",
            description: "Makes cursor/aim movement 1:1 with your hand speed, which is what raw-input games already do. Applies immediately.",
            risk: Risk::Low,
            recommended: true,
            restart: Restart::None,
            actions: vec![
                cu(r"Control Panel\Mouse", "MouseSpeed", S("0"), Some(S("1"))),
                cu(r"Control Panel\Mouse", "MouseThreshold1", S("0"), Some(S("6"))),
                cu(r"Control Panel\Mouse", "MouseThreshold2", S("0"), Some(S("10"))),
            ],
            live: Some(mouse_live),
        },
        Tweak {
            id: "mmcss_games",
            category: Category::Gaming,
            title: "Prioritise games in the multimedia scheduler (MMCSS)",
            description: "Raises the CPU and storage priority Windows gives to threads registered as 'Games'. Helps titles that use MMCSS; harmless for those that don't.",
            risk: Risk::Low,
            recommended: true,
            restart: Restart::Reboot,
            actions: vec![
                lm(MM_GAMES, "GPU Priority", D(8), Some(D(8))),
                lm(MM_GAMES, "Priority", D(6), Some(D(2))),
                lm(MM_GAMES, "Scheduling Category", S("High"), Some(S("Medium"))),
                lm(MM_GAMES, "SFIO Priority", S("High"), Some(S("Normal"))),
            ],
            live: None,
        },
        Tweak {
            id: "valorant_launch_priority",
            category: Category::Gaming,
            title: "VALORANT: start with High CPU priority",
            description: "Tells Windows to launch VALORANT-Win64-Shipping.exe at High priority. This is the anti-cheat-friendly method: Avix never opens or touches the game process. Harmless if VALORANT isn't installed.",
            risk: Risk::Low,
            recommended: true,
            restart: Restart::None,
            actions: vec![lm(
                r"SOFTWARE\Microsoft\Windows NT\CurrentVersion\Image File Execution Options\VALORANT-Win64-Shipping.exe\PerfOptions",
                "CpuPriorityClass",
                D(3),
                None,
            )],
            live: None,
        },
        Tweak {
            id: "fso_off",
            category: Category::Gaming,
            title: "Prefer true exclusive fullscreen (disable fullscreen optimizations)",
            description: "Some older DX9/DX11 games get lower input lag in real exclusive fullscreen. On modern Windows 11 the default is usually as good or better - try it only if a specific game feels floaty.",
            risk: Risk::Medium,
            recommended: false,
            restart: Restart::SignOut,
            actions: vec![
                cu(r"System\GameConfigStore", "GameDVR_FSEBehavior", D(2), None),
                cu(r"System\GameConfigStore", "GameDVR_HonorUserFSEBehaviorMode", D(1), Some(D(0))),
                cu(r"System\GameConfigStore", "GameDVR_DXGIHonorFSEWindowsCompatible", D(1), Some(D(0))),
            ],
            live: None,
        },
        Tweak {
            id: "mpo_off",
            category: Category::Gaming,
            title: "Disable Multiplane Overlay (MPO)",
            description: "Fixes flicker, black screens and stutter on some multi-monitor / mixed refresh-rate setups (often NVIDIA). Only useful if you actually have those symptoms; recent Windows builds may ignore it.",
            risk: Risk::Medium,
            recommended: false,
            restart: Restart::Reboot,
            actions: vec![lm(r"SOFTWARE\Microsoft\Windows\Dwm", "OverlayTestMode", D(5), None)],
            live: None,
        },
        Tweak {
            id: "global_timer",
            category: Category::Gaming,
            title: "Allow global timer-resolution requests",
            description: "Since Windows 10 2004 a 0.5 ms timer request only affects the app that made it. This restores the old global behaviour, so Avix's 'timer resolution' profile option reaches your game. Slightly higher idle power use.",
            risk: Risk::Medium,
            recommended: false,
            restart: Restart::Reboot,
            actions: vec![lm(
                r"SYSTEM\CurrentControlSet\Control\Session Manager\kernel",
                "GlobalTimerResolutionRequests",
                D(1),
                None,
            )],
            live: None,
        },
        Tweak {
            id: "hvci_off",
            category: Category::Gaming,
            title: "Turn off Memory Integrity (Core isolation / HVCI)",
            description: "Virtualization-based code integrity can cost ~5-10% in some CPU-bound games. It is a real security feature that blocks malicious drivers - this is a trade-off, not a free win. Some anti-cheats may require it; turn it back on if a game asks.",
            risk: Risk::High,
            recommended: false,
            restart: Restart::Reboot,
            actions: vec![lm(
                r"SYSTEM\CurrentControlSet\Control\DeviceGuard\Scenarios\HypervisorEnforcedCodeIntegrity",
                "Enabled",
                D(0),
                Some(D(1)),
            )],
            live: None,
        },
        // ------------------------------------------------------------ Performance
        Tweak {
            id: "system_responsiveness",
            category: Category::Performance,
            title: "Reserve less CPU for background multimedia",
            description: "Windows reserves 20% of CPU time for low-priority multimedia tasks; 10% gives foreground apps more headroom.",
            risk: Risk::Low,
            recommended: true,
            restart: Restart::Reboot,
            actions: vec![lm(MM, "SystemResponsiveness", D(10), Some(D(20)))],
            live: None,
        },
        Tweak {
            id: "fast_startup_off",
            category: Category::Performance,
            title: "Disable Fast Startup",
            description: "Fast Startup half-hibernates the kernel and drivers, so 'shut down' never gives you a clean driver start. Disabling it fixes many odd driver/USB/network issues at the cost of a slightly slower boot.",
            risk: Risk::Low,
            recommended: false,
            restart: Restart::None,
            actions: vec![lm(r"SYSTEM\CurrentControlSet\Control\Session Manager\Power", "HiberbootEnabled", D(0), Some(D(1)))],
            live: None,
        },
        Tweak {
            id: "background_apps_off",
            category: Category::Performance,
            title: "Stop Store apps running in the background",
            description: "Prevents UWP/Store apps from running when closed. Store apps that rely on background notifications (e.g. WhatsApp from the Store) will only notify while open.",
            risk: Risk::Medium,
            recommended: false,
            restart: Restart::SignOut,
            actions: vec![lm(r"SOFTWARE\Policies\Microsoft\Windows\AppPrivacy", "LetAppsRunInBackground", D(2), None)],
            live: None,
        },
        Tweak {
            id: "transparency_off",
            category: Category::Performance,
            title: "Turn off transparency effects",
            description: "Removes acrylic blur from the taskbar and menus. A small saving on the desktop compositor; mostly cosmetic.",
            risk: Risk::Low,
            recommended: false,
            restart: Restart::None,
            actions: vec![cu(
                r"Software\Microsoft\Windows\CurrentVersion\Themes\Personalize",
                "EnableTransparency",
                D(0),
                Some(D(1)),
            )],
            live: None,
        },
        Tweak {
            id: "edge_background_off",
            category: Category::Performance,
            title: "Stop Microsoft Edge running in the background",
            description: "Disables Edge Startup Boost and background mode so Edge isn't preloaded and idling in RAM. (Edge will show 'managed by your organization' - that's just this policy.)",
            risk: Risk::Low,
            recommended: true,
            restart: Restart::None,
            actions: vec![
                lm(r"SOFTWARE\Policies\Microsoft\Edge", "StartupBoostEnabled", D(0), None),
                lm(r"SOFTWARE\Policies\Microsoft\Edge", "BackgroundModeEnabled", D(0), None),
            ],
            live: None,
        },
        Tweak {
            id: "widgets_off",
            category: Category::Performance,
            title: "Disable Widgets (news feed)",
            description: "Removes the Widgets board and its background WebView processes.",
            risk: Risk::Low,
            recommended: true,
            restart: Restart::SignOut,
            actions: vec![lm(r"SOFTWARE\Policies\Microsoft\Dsh", "AllowNewsAndInterests", D(0), None)],
            live: None,
        },
        // ------------------------------------------------------------ Network
        Tweak {
            id: "network_throttling_off",
            category: Category::Network,
            title: "Disable multimedia network throttling",
            description: "Windows throttles non-multimedia network packets while media plays (NetworkThrottlingIndex). Turning it off avoids throughput dips while music/streams run in the background.",
            risk: Risk::Low,
            recommended: true,
            restart: Restart::Reboot,
            actions: vec![lm(MM, "NetworkThrottlingIndex", D(0xFFFF_FFFF), Some(D(10)))],
            live: None,
        },
        Tweak {
            id: "delivery_optimization_p2p_off",
            category: Category::Network,
            title: "Stop uploading Windows updates to other PCs",
            description: "Delivery Optimization can use your upload bandwidth to seed updates to strangers on the internet - bad for ping. Updates still download normally from Microsoft.",
            risk: Risk::Low,
            recommended: true,
            restart: Restart::None,
            actions: vec![lm(r"SOFTWARE\Policies\Microsoft\Windows\DeliveryOptimization", "DODownloadMode", D(0), None)],
            live: None,
        },
        Tweak {
            id: "nagle_off",
            category: Category::Network,
            title: "Disable Nagle's algorithm (TCP no-delay)",
            description: "Sends small TCP packets immediately instead of batching them. Helps TCP-based games (some MMOs, League's client). VALORANT, CS2 and most shooters use UDP, so it won't change their ping.",
            risk: Risk::Low,
            recommended: false,
            restart: Restart::Reboot,
            actions: vec![
                Action::RegTcpInterfaces { name: "TcpAckFrequency", value: 1 },
                Action::RegTcpInterfaces { name: "TCPNoDelay", value: 1 },
            ],
            live: None,
        },
        // ------------------------------------------------------------ Privacy
        Tweak {
            id: "telemetry_min",
            category: Category::Privacy,
            title: "Minimise diagnostic data (telemetry)",
            description: "Sets the diagnostic-data policy to the lowest level your edition allows (Required/Security).",
            risk: Risk::Low,
            recommended: true,
            restart: Restart::None,
            actions: vec![
                lm(DATACOLLECTION, "AllowTelemetry", D(0), None),
                lm(DATACOLLECTION, "DoNotShowFeedbackNotifications", D(1), None),
            ],
            live: None,
        },
        Tweak {
            id: "telemetry_services",
            category: Category::Privacy,
            title: "Disable the telemetry services",
            description: "Connected User Experiences & Telemetry (DiagTrack) and the WAP push routing service. Nothing user-facing depends on them.",
            risk: Risk::Low,
            recommended: true,
            restart: Restart::None,
            actions: vec![svc("DiagTrack", START_DISABLED, START_AUTO), svc("dmwappushservice", START_DISABLED, START_MANUAL)],
            live: None,
        },
        Tweak {
            id: "telemetry_tasks",
            category: Category::Privacy,
            title: "Disable telemetry scheduled tasks",
            description: "Compatibility Appraiser (CompatTelRunner.exe - a well-known source of random CPU/disk spikes), CEIP, disk-diagnostic and feedback uploaders.",
            risk: Risk::Low,
            recommended: true,
            restart: Restart::None,
            actions: vec![
                task(r"\Microsoft\Windows\Application Experience\Microsoft Compatibility Appraiser"),
                task(r"\Microsoft\Windows\Application Experience\Microsoft Compatibility Appraiser Exp"),
                task(r"\Microsoft\Windows\Application Experience\ProgramDataUpdater"),
                task(r"\Microsoft\Windows\Application Experience\MareBackup"),
                task(r"\Microsoft\Windows\Autochk\Proxy"),
                task(r"\Microsoft\Windows\Customer Experience Improvement Program\Consolidator"),
                task(r"\Microsoft\Windows\Customer Experience Improvement Program\UsbCeip"),
                task(r"\Microsoft\Windows\DiskDiagnostic\Microsoft-Windows-DiskDiagnosticDataCollector"),
                task(r"\Microsoft\Windows\Feedback\Siuf\DmClient"),
                task(r"\Microsoft\Windows\Feedback\Siuf\DmClientOnScenarioDownload"),
                task(r"\Microsoft\Windows\Windows Error Reporting\QueueReporting"),
            ],
            live: None,
        },
        Tweak {
            id: "copilot_off",
            category: Category::Privacy,
            title: "Turn off Copilot",
            description: "Disables Windows Copilot via policy and hides its taskbar button.",
            risk: Risk::Low,
            recommended: true,
            restart: Restart::Explorer,
            actions: vec![
                cu(r"Software\Policies\Microsoft\Windows\WindowsCopilot", "TurnOffWindowsCopilot", D(1), None),
                lm(r"SOFTWARE\Policies\Microsoft\Windows\WindowsCopilot", "TurnOffWindowsCopilot", D(1), None),
                cu(ADV, "ShowCopilotButton", D(0), None),
            ],
            live: None,
        },
        Tweak {
            id: "recall_off",
            category: Category::Privacy,
            title: "Turn off Recall (AI snapshots)",
            description: "Stops Windows from taking and analysing periodic screenshots of your activity on Copilot+ PCs.",
            risk: Risk::Low,
            recommended: true,
            restart: Restart::Reboot,
            actions: vec![
                lm(r"SOFTWARE\Policies\Microsoft\Windows\WindowsAI", "DisableAIDataAnalysis", D(1), None),
                lm(r"SOFTWARE\Policies\Microsoft\Windows\WindowsAI", "AllowRecallEnablement", D(0), None),
                cu(r"Software\Policies\Microsoft\Windows\WindowsAI", "DisableAIDataAnalysis", D(1), None),
            ],
            live: None,
        },
        Tweak {
            id: "advertising_id_off",
            category: Category::Privacy,
            title: "Disable the advertising ID",
            description: "Stops apps from using a per-user ID to personalise ads.",
            risk: Risk::Low,
            recommended: true,
            restart: Restart::None,
            actions: vec![
                cu(r"Software\Microsoft\Windows\CurrentVersion\AdvertisingInfo", "Enabled", D(0), None),
                lm(r"SOFTWARE\Policies\Microsoft\Windows\AdvertisingInfo", "DisabledByGroupPolicy", D(1), None),
            ],
            live: None,
        },
        Tweak {
            id: "tailored_experiences_off",
            category: Category::Privacy,
            title: "Disable tailored experiences",
            description: "Stops Microsoft using your diagnostic data for personalised tips, ads and recommendations.",
            risk: Risk::Low,
            recommended: true,
            restart: Restart::None,
            actions: vec![cu(
                r"Software\Microsoft\Windows\CurrentVersion\Privacy",
                "TailoredExperiencesWithDiagnosticDataEnabled",
                D(0),
                None,
            )],
            live: None,
        },
        Tweak {
            id: "activity_history_off",
            category: Category::Privacy,
            title: "Disable activity history",
            description: "Stops Windows collecting and uploading a timeline of the apps and files you use.",
            risk: Risk::Low,
            recommended: true,
            restart: Restart::None,
            actions: vec![
                lm(r"SOFTWARE\Policies\Microsoft\Windows\System", "EnableActivityFeed", D(0), None),
                lm(r"SOFTWARE\Policies\Microsoft\Windows\System", "PublishUserActivities", D(0), None),
                lm(r"SOFTWARE\Policies\Microsoft\Windows\System", "UploadUserActivities", D(0), None),
            ],
            live: None,
        },
        Tweak {
            id: "bing_search_off",
            category: Category::Privacy,
            title: "Remove Bing web results from Start search",
            description: "Start menu search only searches your PC - faster, and your keystrokes aren't sent to Bing.",
            risk: Risk::Low,
            recommended: true,
            restart: Restart::Explorer,
            actions: vec![
                cu(r"Software\Policies\Microsoft\Windows\Explorer", "DisableSearchBoxSuggestions", D(1), None),
                cu(r"Software\Microsoft\Windows\CurrentVersion\Search", "BingSearchEnabled", D(0), None),
                lm(r"SOFTWARE\Policies\Microsoft\Windows\Windows Search", "AllowCortana", D(0), None),
            ],
            live: None,
        },
        Tweak {
            id: "error_reporting_off",
            category: Category::Privacy,
            title: "Disable Windows Error Reporting uploads",
            description: "Crash reports are no longer collected/uploaded (WerFault won't pop up and churn disk after a crash).",
            risk: Risk::Low,
            recommended: false,
            restart: Restart::None,
            actions: vec![
                lm(r"SOFTWARE\Microsoft\Windows\Windows Error Reporting", "Disabled", D(1), None),
                svc("WerSvc", START_DISABLED, START_MANUAL),
            ],
            live: None,
        },
        // ------------------------------------------------------------ Ads & nags
        Tweak {
            id: "start_ads_off",
            category: Category::Ads,
            title: "Stop silently installed apps & Start menu ads",
            description: "Stops Windows auto-installing 'suggested' apps (Candy Crush etc.) and removes promoted tiles, Start recommendations and Settings suggestions.",
            risk: Risk::Low,
            recommended: true,
            restart: Restart::Explorer,
            actions: vec![
                cu(CDM, "SilentInstalledAppsEnabled", D(0), Some(D(1))),
                cu(CDM, "PreInstalledAppsEnabled", D(0), Some(D(1))),
                cu(CDM, "OemPreInstalledAppsEnabled", D(0), Some(D(1))),
                cu(CDM, "SystemPaneSuggestionsEnabled", D(0), Some(D(1))),
                cu(CDM, "SubscribedContent-338388Enabled", D(0), Some(D(1))),
                cu(CDM, "SubscribedContent-338393Enabled", D(0), Some(D(1))),
                cu(CDM, "SubscribedContent-353694Enabled", D(0), Some(D(1))),
                cu(CDM, "SubscribedContent-353696Enabled", D(0), Some(D(1))),
                cu(ADV, "Start_IrisRecommendations", D(0), None),
                lm(r"SOFTWARE\Policies\Microsoft\Windows\CloudContent", "DisableWindowsConsumerFeatures", D(1), None),
            ],
            live: None,
        },
        Tweak {
            id: "lockscreen_tips_off",
            category: Category::Ads,
            title: "Remove lock-screen tips & 'fun facts'",
            description: "Stops Spotlight tips and promotions on the lock screen and 'Get tips and suggestions' notifications.",
            risk: Risk::Low,
            recommended: true,
            restart: Restart::None,
            actions: vec![
                cu(CDM, "RotatingLockScreenOverlayEnabled", D(0), Some(D(1))),
                cu(CDM, "SubscribedContent-338389Enabled", D(0), Some(D(1))),
                cu(CDM, "SoftLandingEnabled", D(0), Some(D(1))),
                cu(CDM, "SubscribedContent-310093Enabled", D(0), Some(D(1))),
            ],
            live: None,
        },
        Tweak {
            id: "finish_setup_nag_off",
            category: Category::Ads,
            title: "Stop 'Let's finish setting up your device' nags",
            description: "Removes the full-screen OneDrive / Microsoft 365 / Game Pass upsell that appears after updates.",
            risk: Risk::Low,
            recommended: true,
            restart: Restart::None,
            actions: vec![cu(
                r"Software\Microsoft\Windows\CurrentVersion\UserProfileEngagement",
                "ScoobeSystemSettingEnabled",
                D(0),
                None,
            )],
            live: None,
        },
        Tweak {
            id: "feedback_prompts_off",
            category: Category::Ads,
            title: "Never ask for feedback",
            description: "Stops 'How likely are you to recommend Windows?' pop-ups.",
            risk: Risk::Low,
            recommended: true,
            restart: Restart::None,
            actions: vec![cu(r"Software\Microsoft\Siuf\Rules", "NumberOfSIUFInPeriod", D(0), None)],
            live: None,
        },
        // ------------------------------------------------------------ Services
        Tweak {
            id: "svc_maps_broker",
            category: Category::Services,
            title: "Disable Downloaded Maps Manager",
            description: "Keeps offline maps updated in the background. Not needed unless you use the Maps app offline.",
            risk: Risk::Low,
            recommended: true,
            restart: Restart::None,
            actions: vec![svc("MapsBroker", START_DISABLED, START_AUTO)],
            live: None,
        },
        Tweak {
            id: "svc_retail_demo",
            category: Category::Services,
            title: "Disable Retail Demo service",
            description: "Only used by store display PCs.",
            risk: Risk::Low,
            recommended: true,
            restart: Restart::None,
            actions: vec![svc("RetailDemo", START_DISABLED, START_MANUAL)],
            live: None,
        },
        Tweak {
            id: "svc_fax",
            category: Category::Services,
            title: "Disable Fax service",
            description: "You don't have a fax modem.",
            risk: Risk::Low,
            recommended: true,
            restart: Restart::None,
            actions: vec![svc("Fax", START_DISABLED, START_MANUAL)],
            live: None,
        },
        Tweak {
            id: "svc_sysmain",
            category: Category::Services,
            title: "Disable SysMain (Superfetch)",
            description: "Preloads apps into RAM. On fast NVMe SSDs the benefit is small and it can cause disk/CPU activity in the background. Debated - leave on if you have a hard drive or little RAM.",
            risk: Risk::Medium,
            recommended: false,
            restart: Restart::None,
            actions: vec![svc("SysMain", START_DISABLED, START_AUTO)],
            live: None,
        },
        Tweak {
            id: "svc_search_index",
            category: Category::Services,
            title: "Disable Windows Search indexing",
            description: "Stops the background indexer (SearchIndexer.exe). Start/File Explorer search still works but becomes slower for file contents; Outlook search is affected.",
            risk: Risk::Medium,
            recommended: false,
            restart: Restart::None,
            actions: vec![svc("WSearch", START_DISABLED, START_AUTO)],
            live: None,
        },
        Tweak {
            id: "svc_print_spooler",
            category: Category::Services,
            title: "Disable Print Spooler",
            description: "Only if you never print (also closes a historically vulnerable service). Re-enable before printing or using 'Print to PDF'.",
            risk: Risk::Medium,
            recommended: false,
            restart: Restart::None,
            actions: vec![svc("Spooler", START_DISABLED, START_AUTO)],
            live: None,
        },
    ]
}
