//! Preinstalled Store-app removal (via Windows PowerShell's Appx module) and OneDrive.

use crate::sys::{self, registry, registry::Hive};
use serde::Deserialize;

pub struct BloatApp {
    pub pattern: &'static str,
    pub label: &'static str,
    pub note: &'static str,
    pub recommended: bool,
}

const fn app(pattern: &'static str, label: &'static str, note: &'static str, recommended: bool) -> BloatApp {
    BloatApp { pattern, label, note, recommended }
}

/// Deliberately excludes things games or Windows need (Xbox Identity Provider, Xbox TCUI,
/// Store, App Installer, VCLibs/.NET frameworks, Calculator, Photos, Notepad, Terminal...).
pub const BLOAT: &[BloatApp] = &[
    app("Microsoft.BingNews", "Microsoft News", "", true),
    app("Microsoft.BingWeather", "MSN Weather", "", true),
    app("Microsoft.BingSearch", "Bing Search", "", true),
    app("Microsoft.GetHelp", "Get Help", "", true),
    app("Microsoft.Getstarted", "Tips", "", true),
    app("Microsoft.MicrosoftSolitaireCollection", "Solitaire Collection", "", true),
    app("Microsoft.MicrosoftOfficeHub", "Microsoft 365 (Office) hub", "A launcher/upsell for Office web apps", true),
    app("Microsoft.People", "People", "", true),
    app("Microsoft.WindowsFeedbackHub", "Feedback Hub", "", true),
    app("Microsoft.WindowsMaps", "Maps", "", true),
    app("Microsoft.ZuneVideo", "Movies & TV", "", true),
    app("Microsoft.SkypeApp", "Skype", "", true),
    app("MicrosoftTeams", "Teams (personal, classic)", "", true),
    app("Clipchamp.Clipchamp", "Clipchamp", "", true),
    app("Microsoft.PowerAutomateDesktop", "Power Automate", "", true),
    app("Microsoft.549981C3F5F10", "Cortana", "", true),
    app("Microsoft.Copilot", "Copilot app", "", true),
    app("Microsoft.Windows.DevHome", "Dev Home", "", true),
    app("Microsoft.MixedReality.Portal", "Mixed Reality Portal", "", true),
    app("Microsoft.Microsoft3DViewer", "3D Viewer", "", true),
    app("Microsoft.Print3D", "Print 3D", "", true),
    app("Microsoft.MSPaint", "Paint 3D", "Classic Paint is not affected", true),
    app("Microsoft.Wallet", "Wallet", "", true),
    app("Microsoft.Messaging", "Messaging", "", true),
    app("Microsoft.OneConnect", "Mobile Plans", "", true),
    app("MicrosoftCorporationII.MicrosoftFamily", "Microsoft Family", "", true),
    app("Microsoft.YourPhone", "Phone Link", "Keeps a background process running", true),
    app(
        "MicrosoftWindows.Client.WebExperience",
        "Widgets (Web Experience Pack)",
        "Removes the Widgets board and its background processes",
        true,
    ),
    app("Microsoft.XboxSpeechToTextOverlay", "Xbox speech-to-text overlay", "", true),
    app("king.com.*", "Candy Crush & other King games", "", true),
    app("Disney.37853FC22B2CE", "Disney+", "", true),
    app("Facebook.*", "Facebook / Instagram", "", true),
    app("BytedancePte.Ltd.TikTok", "TikTok", "", true),
    app("AmazonVideo.PrimeVideo", "Prime Video", "", true),
    app("5A894077.McAfeeSecurity", "McAfee promo", "", true),
    app("7EE7776C.LinkedInforWindows", "LinkedIn", "", true),
    app("MSTeams", "Teams (new)", "Remove only if you don't use Teams", false),
    app("Microsoft.OutlookForWindows", "Outlook (new)", "Remove only if you use another mail app", false),
    app("microsoft.windowscommunicationsapps", "Mail & Calendar (legacy)", "", false),
    app("Microsoft.Todos", "Microsoft To Do", "", false),
    app("Microsoft.WindowsAlarms", "Clock & Alarms", "", false),
    app("Microsoft.WindowsSoundRecorder", "Sound Recorder", "", false),
    app("Microsoft.MicrosoftStickyNotes", "Sticky Notes", "", false),
    app("Microsoft.ZuneMusic", "Media Player", "Windows' default music/video player", false),
    app("Microsoft.XboxGamingOverlay", "Xbox Game Bar", "Some games use Win+G features", false),
    app("Microsoft.GamingApp", "Xbox app", "Needed for PC Game Pass", false),
    app("SpotifyAB.SpotifyMusic", "Spotify (Store)", "Remove only if you don't use it", false),
    app("MicrosoftCorporationII.QuickAssist", "Quick Assist", "", false),
];

#[derive(Clone, Debug, Deserialize)]
pub struct InstalledPkg {
    #[serde(rename = "Name")]
    pub name: String,
    #[serde(rename = "NonRemovable", default)]
    pub non_removable: Option<bool>,
}

pub fn glob_match(pattern: &str, text: &str) -> bool {
    let (p, t) = (pattern.to_lowercase(), text.to_lowercase());
    match p.split_once('*') {
        None => p == t,
        Some((pre, post)) => t.len() >= pre.len() + post.len() && t.starts_with(pre) && t.ends_with(post),
    }
}

fn ps_list(items: &[String]) -> String {
    items.iter().map(|s| format!("'{}'", s.replace('\'', "''"))).collect::<Vec<_>>().join(",")
}

/// Installed (non-framework) packages. All users when elevated.
pub fn list_installed() -> Result<Vec<InstalledPkg>, String> {
    let all = if sys::elevation::is_elevated() { "-AllUsers" } else { "" };
    let script = format!(
        "$p = Get-AppxPackage {all} | Where-Object {{ -not $_.IsFramework }} | Select-Object Name,NonRemovable -Unique; ConvertTo-Json -InputObject @($p) -Compress"
    );
    let out = sys::powershell(&script)?;
    if out.trim().is_empty() {
        return Ok(Vec::new());
    }
    let mut v: Vec<InstalledPkg> = serde_json::from_str(&out).map_err(|e| format!("couldn't parse app list: {e}"))?;
    v.sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase()));
    v.dedup_by(|a, b| a.name.eq_ignore_ascii_case(&b.name));
    Ok(v)
}

#[derive(Clone, Debug, Deserialize)]
pub struct RemoveResult {
    pub name: String,
    pub ok: bool,
    pub error: String,
}

/// Remove packages by name/pattern for all users and stop them being provisioned to new users.
pub fn remove(names: &[String]) -> Result<Vec<RemoveResult>, String> {
    let elevated = sys::elevation::is_elevated();
    let all = if elevated { "-AllUsers" } else { "" };
    // Removing for existing users is what matters; de-provisioning (so new accounts don't
    // get the app) is best-effort and reported as a note, never as a failure.
    let script = format!(
        r#"$names = @({list}); $r = @(); $elevated = ${elevated}
$prov = @(); $provErr = ''
if ($elevated) {{ try {{ $prov = @(Get-AppxProvisionedPackage -Online -ErrorAction Stop) }} catch {{ $provErr = 'new-account cleanup skipped: ' + $_.Exception.Message.Trim() }} }}
foreach ($n in $names) {{
  $errs = @(); $notes = @()
  try {{
    $pk = @(Get-AppxPackage {all} -Name $n -ErrorAction Stop)
    foreach ($p in $pk) {{ Remove-AppxPackage -Package $p.PackageFullName {all} -ErrorAction Stop }}
  }} catch {{ $errs += $_.Exception.Message.Trim() }}
  if ($provErr) {{ $notes += $provErr }}
  foreach ($pp in ($prov | Where-Object {{ $_.DisplayName -like $n }})) {{
    try {{ Remove-AppxProvisionedPackage -Online -PackageName $pp.PackageName -ErrorAction Stop | Out-Null }}
    catch {{ $notes += 'new-account cleanup: ' + $_.Exception.Message.Trim() }}
  }}
  $r += [pscustomobject]@{{ name = $n; ok = ($errs.Count -eq 0); error = (($errs + $notes) -join '; ') }}
}}
ConvertTo-Json -InputObject @($r) -Compress"#,
        list = ps_list(names),
        elevated = if elevated { "true" } else { "false" }
    );
    let out = sys::powershell(&script)?;
    serde_json::from_str(&out).map_err(|e| format!("unexpected PowerShell output: {e}: {out}"))
}

// ---------------------------------------------------------------- OneDrive

const ONEDRIVE_UNINSTALL: &str = r"Software\Microsoft\Windows\CurrentVersion\Uninstall\OneDriveSetup.exe";

pub fn onedrive_uninstall_string() -> Option<String> {
    registry::get_string(Hive::HKCU, ONEDRIVE_UNINSTALL, "UninstallString")
        .or_else(|| registry::get_string(Hive::HKLM, ONEDRIVE_UNINSTALL, "UninstallString"))
}

pub fn uninstall_onedrive() -> Result<(), String> {
    use std::os::windows::process::CommandExt;
    let cmd = onedrive_uninstall_string().ok_or("OneDrive is not installed")?;
    let _ = sys::hidden_cmd(&sys::system32("taskkill.exe")).args(["/F", "/IM", "OneDrive.exe"]).output();
    let cmd = cmd.trim();
    let (exe, args) = if let Some(rest) = cmd.strip_prefix('"') {
        let end = rest.find('"').ok_or("bad uninstall string")?;
        (rest[..end].to_string(), rest[end + 1..].trim().to_string())
    } else {
        match cmd.split_once(' ') {
            Some((e, a)) => (e.to_string(), a.to_string()),
            None => (cmd.to_string(), String::new()),
        }
    };
    let status = sys::hidden_cmd(&exe).raw_arg(&args).status().map_err(|e| e.to_string())?;
    if status.success() {
        Ok(())
    } else {
        Err(format!("OneDrive uninstaller exited with {status}"))
    }
}
