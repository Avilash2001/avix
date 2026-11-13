//! UAC elevation helpers.

use super::{quote_arg, wide};
use std::ffi::c_void;
use windows_sys::Win32::Foundation::{CloseHandle, HANDLE};
use windows_sys::Win32::Security::{GetTokenInformation, TokenElevation, TOKEN_ELEVATION, TOKEN_QUERY};
use windows_sys::Win32::System::Threading::{GetCurrentProcess, OpenProcessToken};
use windows_sys::Win32::UI::Shell::ShellExecuteW;
use windows_sys::Win32::UI::WindowsAndMessaging::SW_SHOWNORMAL;

pub fn is_elevated() -> bool {
    unsafe {
        let mut token: HANDLE = std::ptr::null_mut();
        if OpenProcessToken(GetCurrentProcess(), TOKEN_QUERY, &mut token) == 0 {
            return false;
        }
        let mut e = TOKEN_ELEVATION { TokenIsElevated: 0 };
        let mut ret = 0u32;
        let ok = GetTokenInformation(
            token,
            TokenElevation,
            &mut e as *mut _ as *mut c_void,
            std::mem::size_of::<TOKEN_ELEVATION>() as u32,
            &mut ret,
        );
        CloseHandle(token);
        ok != 0 && e.TokenIsElevated != 0
    }
}

/// Start this executable again through the UAC prompt. Returns false if the user declined.
pub fn relaunch_elevated(args: &[String]) -> bool {
    let Ok(exe) = std::env::current_exe() else { return false };
    run_elevated(&exe.to_string_lossy(), &args.iter().map(|a| quote_arg(a)).collect::<Vec<_>>().join(" "))
}

pub fn run_elevated(exe: &str, params: &str) -> bool {
    let verb = wide("runas");
    let file = wide(exe);
    let p = wide(params);
    let r = unsafe {
        ShellExecuteW(
            std::ptr::null_mut(),
            verb.as_ptr(),
            file.as_ptr(),
            p.as_ptr(),
            std::ptr::null(),
            SW_SHOWNORMAL,
        )
    };
    r as isize > 32
}

/// Open a file, folder or URL with its default handler.
pub fn shell_open(target: &str) {
    let verb = wide("open");
    let file = wide(target);
    unsafe {
        ShellExecuteW(
            std::ptr::null_mut(),
            verb.as_ptr(),
            file.as_ptr(),
            std::ptr::null(),
            std::ptr::null(),
            SW_SHOWNORMAL,
        );
    }
}

/// Open a web page in the user's normal browser. Handing it to Explorer means the browser starts
/// un-elevated even though Avix runs as admin.
pub fn open_url(url: &str) {
    if url.starts_with("https://") {
        let _ = std::process::Command::new("explorer.exe").arg(url).spawn();
    }
}
