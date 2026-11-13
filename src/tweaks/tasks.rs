//! Scheduled-task enable/disable via schtasks.exe (its XML output is not localized).

use crate::sys::{hidden_cmd, system32};

fn decode(b: &[u8]) -> String {
    let utf16 = (b.len() >= 2 && b[0] == 0xFF && b[1] == 0xFE)
        || (b.len() > 16 && b.iter().skip(1).step_by(2).take(32).all(|&x| x == 0));
    if utf16 {
        let u: Vec<u16> = b.chunks_exact(2).map(|c| u16::from_le_bytes([c[0], c[1]])).collect();
        String::from_utf16_lossy(&u)
    } else {
        String::from_utf8_lossy(b).into_owned()
    }
}

/// None if the task doesn't exist (or can't be read).
pub fn is_enabled(path: &str) -> Option<bool> {
    let out = hidden_cmd(&system32("schtasks.exe")).args(["/Query", "/TN", path, "/XML"]).output().ok()?;
    if !out.status.success() {
        return None;
    }
    let xml = decode(&out.stdout);
    let settings = xml.split("<Settings>").nth(1).and_then(|s| s.split("</Settings>").next()).unwrap_or("");
    Some(!settings.contains("<Enabled>false</Enabled>"))
}

pub fn set_enabled(path: &str, enabled: bool) -> Result<(), String> {
    let out = hidden_cmd(&system32("schtasks.exe"))
        .args(["/Change", "/TN", path, if enabled { "/ENABLE" } else { "/DISABLE" }])
        .output()
        .map_err(|e| e.to_string())?;
    if out.status.success() {
        Ok(())
    } else {
        let msg = decode(&out.stderr);
        Err(format!("{}: {}", path.rsplit('\\').next().unwrap_or(path), msg.trim()))
    }
}
