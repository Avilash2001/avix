//! Foreground-window inspection and fullscreen detection.

use super::wide;
use std::ffi::c_void;
use windows_sys::Win32::Foundation::{BOOL, HWND, LPARAM, RECT};
use windows_sys::Win32::Graphics::Dwm::{DwmGetWindowAttribute, DWMWA_CLOAKED};
use windows_sys::Win32::Graphics::Gdi::{GetMonitorInfoW, MonitorFromWindow, MONITORINFO, MONITOR_DEFAULTTONEAREST};
use windows_sys::Win32::UI::WindowsAndMessaging::{
    EnumWindows, FindWindowW, GetClassNameW, GetDesktopWindow, GetForegroundWindow, GetShellWindow,
    GetWindow, GetWindowLongPtrW, GetWindowRect, GetWindowThreadProcessId, IsIconic, IsWindowVisible,
    SetForegroundWindow, ShowWindow, GWL_EXSTYLE, GW_OWNER, SW_RESTORE, WS_EX_NOACTIVATE, WS_EX_TOOLWINDOW,
    WS_EX_TRANSPARENT,
};

/// A real application window on screen.
#[derive(Clone, Copy, Debug)]
pub struct AppWindow {
    pub pid: u32,
    pub width: i32,
    pub height: i32,
    pub fullscreen: bool,
}

/// Visible top-level app windows: not minimised, not cloaked (hidden UWP / other desktops),
/// not owned popups, and not tool / click-through / non-activating overlays.
pub fn app_windows() -> Vec<AppWindow> {
    unsafe extern "system" fn each(h: HWND, lp: LPARAM) -> BOOL {
        let out = &mut *(lp as *mut Vec<AppWindow>);
        if IsWindowVisible(h) == 0 || IsIconic(h) != 0 || !GetWindow(h, GW_OWNER).is_null() {
            return 1;
        }
        let ex = GetWindowLongPtrW(h, GWL_EXSTYLE) as u32;
        if ex & (WS_EX_TOOLWINDOW | WS_EX_TRANSPARENT | WS_EX_NOACTIVATE) != 0 {
            return 1;
        }
        let mut cloaked = 0u32;
        if DwmGetWindowAttribute(h, DWMWA_CLOAKED as _, &mut cloaked as *mut u32 as *mut c_void, 4) == 0 && cloaked != 0 {
            return 1;
        }
        let mut r: RECT = std::mem::zeroed();
        if GetWindowRect(h, &mut r) == 0 {
            return 1;
        }
        let (width, height) = (r.right - r.left, r.bottom - r.top);
        if width < 200 || height < 150 {
            return 1;
        }
        let mut pid = 0u32;
        GetWindowThreadProcessId(h, &mut pid);
        if pid != 0 {
            out.push(AppWindow { pid, width, height, fullscreen: is_fullscreen(h) });
        }
        1
    }
    let mut v: Vec<AppWindow> = Vec::new();
    unsafe {
        EnumWindows(Some(each), &mut v as *mut Vec<AppWindow> as LPARAM);
    }
    v
}

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
