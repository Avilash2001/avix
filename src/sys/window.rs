//! Foreground-window inspection and fullscreen detection.

use super::wide;
use windows_sys::Win32::Foundation::{HWND, RECT};
use windows_sys::Win32::Graphics::Gdi::{GetMonitorInfoW, MonitorFromWindow, MONITORINFO, MONITOR_DEFAULTTONEAREST};
use windows_sys::Win32::UI::WindowsAndMessaging::{
    FindWindowW, GetClassNameW, GetDesktopWindow, GetForegroundWindow, GetShellWindow,
    GetWindowRect, GetWindowThreadProcessId, IsIconic, IsWindowVisible, SetForegroundWindow,
    ShowWindow, SW_RESTORE,
};

pub fn foreground() -> Option<(HWND, u32)> {
    unsafe {
        let h = GetForegroundWindow();
        if h.is_null() {
            return None;
        }
        let mut pid = 0u32;
        GetWindowThreadProcessId(h, &mut pid);
        (pid != 0).then_some((h, pid))
    }
}

fn class_name(h: HWND) -> String {
    let mut buf = [0u16; 128];
    let n = unsafe { GetClassNameW(h, buf.as_mut_ptr(), buf.len() as i32) };
    String::from_utf16_lossy(&buf[..n.max(0) as usize])
}

/// True if the window covers its whole monitor (exclusive or borderless fullscreen).
pub fn is_fullscreen(h: HWND) -> bool {
    unsafe {
        if h.is_null() || h == GetDesktopWindow() || h == GetShellWindow() {
            return false;
        }
        if IsWindowVisible(h) == 0 || IsIconic(h) != 0 {
            return false;
        }
        let cls = class_name(h);
        if matches!(cls.as_str(), "Progman" | "WorkerW" | "Shell_TrayWnd" | "Shell_SecondaryTrayWnd") {
            return false;
        }
        let mut r: RECT = std::mem::zeroed();
        if GetWindowRect(h, &mut r) == 0 {
            return false;
        }
        let mon = MonitorFromWindow(h, MONITOR_DEFAULTTONEAREST);
        let mut mi: MONITORINFO = std::mem::zeroed();
        mi.cbSize = std::mem::size_of::<MONITORINFO>() as u32;
        if GetMonitorInfoW(mon, &mut mi) == 0 {
            return false;
        }
        let m = mi.rcMonitor;
        r.left <= m.left && r.top <= m.top && r.right >= m.right && r.bottom >= m.bottom
    }
}

/// Bring a top-level window with this exact title to the front.
pub fn bring_to_front(title: &str) -> bool {
    let t = wide(title);
    unsafe {
        let h = FindWindowW(std::ptr::null(), t.as_ptr());
        if h.is_null() {
            return false;
        }
        if IsIconic(h) != 0 {
            ShowWindow(h, SW_RESTORE);
        }
        SetForegroundWindow(h);
        true
    }
}
