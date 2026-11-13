//! Windows service control (SCM) with a registry fallback for start types.

use super::{error_text, last_error, registry, wide};
use std::ffi::c_void;
use windows_sys::Win32::Foundation::{
    ERROR_ACCESS_DENIED, ERROR_SERVICE_ALREADY_RUNNING, ERROR_SERVICE_NOT_ACTIVE,
};
use windows_sys::Win32::System::Services::{
    ChangeServiceConfig2W, ChangeServiceConfigW, CloseServiceHandle, ControlService,
    OpenSCManagerW, OpenServiceW, QueryServiceConfig2W, QueryServiceConfigW, QueryServiceStatus,
    StartServiceW, QUERY_SERVICE_CONFIGW, SC_HANDLE, SC_MANAGER_CONNECT,
    SERVICE_CHANGE_CONFIG, SERVICE_CONFIG_DELAYED_AUTO_START_INFO, SERVICE_CONTROL_STOP,
    SERVICE_DELAYED_AUTO_START_INFO, SERVICE_NO_CHANGE, SERVICE_QUERY_CONFIG,
    SERVICE_QUERY_STATUS, SERVICE_RUNNING, SERVICE_START, SERVICE_START_PENDING, SERVICE_STATUS,
    SERVICE_STOP,
};

pub const START_AUTO: u32 = 2;
pub const START_MANUAL: u32 = 3;
pub const START_DISABLED: u32 = 4;

struct Sc(SC_HANDLE);

impl Drop for Sc {
    fn drop(&mut self) {
        unsafe {
            CloseServiceHandle(self.0);
        }
    }
}

fn open(name: &str, access: u32) -> Result<(Sc, Sc), u32> {
    unsafe {
        let m = OpenSCManagerW(std::ptr::null(), std::ptr::null(), SC_MANAGER_CONNECT);
        if m.is_null() {
            return Err(last_error());
        }
        let m = Sc(m);
        let n = wide(name);
        let s = OpenServiceW(m.0, n.as_ptr(), access);
        if s.is_null() {
            return Err(last_error());
        }
        Ok((m, Sc(s)))
    }
}

#[derive(Clone, Copy, Debug)]
pub struct SvcState {
    pub running: bool,
    pub start_type: u32,
    pub delayed: bool,
}

pub fn exists(name: &str) -> bool {
    open(name, SERVICE_QUERY_STATUS).is_ok()
}

pub fn query(name: &str) -> Option<SvcState> {
    let (_m, s) = open(name, SERVICE_QUERY_STATUS | SERVICE_QUERY_CONFIG).ok()?;
    unsafe {
        let mut st: SERVICE_STATUS = std::mem::zeroed();
        QueryServiceStatus(s.0, &mut st);
        let running = st.dwCurrentState == SERVICE_RUNNING || st.dwCurrentState == SERVICE_START_PENDING;

        let mut needed = 0u32;
        QueryServiceConfigW(s.0, std::ptr::null_mut(), 0, &mut needed);
        let mut buf = vec![0u64; (needed as usize / 8) + 2];
        let mut start_type = START_MANUAL;
        if QueryServiceConfigW(s.0, buf.as_mut_ptr() as *mut QUERY_SERVICE_CONFIGW, (buf.len() * 8) as u32, &mut needed) != 0 {
            start_type = (*(buf.as_ptr() as *const QUERY_SERVICE_CONFIGW)).dwStartType;
        }

        let mut delayed = SERVICE_DELAYED_AUTO_START_INFO { fDelayedAutostart: 0 };
        let mut n2 = 0u32;
        QueryServiceConfig2W(
            s.0,
            SERVICE_CONFIG_DELAYED_AUTO_START_INFO,
            &mut delayed as *mut _ as *mut u8,
            std::mem::size_of::<SERVICE_DELAYED_AUTO_START_INFO>() as u32,
            &mut n2,
        );
        Some(SvcState { running, start_type, delayed: delayed.fDelayedAutostart != 0 })
    }
}

pub fn is_running(name: &str) -> bool {
    query(name).is_some_and(|s| s.running)
}

/// Ask a service to stop (does not wait).
pub fn stop(name: &str) -> Result<(), String> {
    let (_m, s) = open(name, SERVICE_STOP | SERVICE_QUERY_STATUS).map_err(error_text)?;
    unsafe {
        let mut st: SERVICE_STATUS = std::mem::zeroed();
        if ControlService(s.0, SERVICE_CONTROL_STOP, &mut st) == 0 {
            let e = last_error();
            if e != ERROR_SERVICE_NOT_ACTIVE {
                return Err(error_text(e));
            }
        }
    }
    Ok(())
}

pub fn start(name: &str) -> Result<(), String> {
    use windows_sys::Win32::System::Services::SERVICE_STOP_PENDING;
    let (_m, s) = open(name, SERVICE_START | SERVICE_QUERY_STATUS).map_err(error_text)?;
    unsafe {
        // A service that is still shutting down rejects StartService - wait for it (max ~8 s).
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(8);
        loop {
            let mut st: SERVICE_STATUS = std::mem::zeroed();
            if QueryServiceStatus(s.0, &mut st) == 0 || st.dwCurrentState != SERVICE_STOP_PENDING {
                break;
            }
            if std::time::Instant::now() > deadline {
                return Err(format!("{name} is still stopping"));
            }
            std::thread::sleep(std::time::Duration::from_millis(200));
        }
        if StartServiceW(s.0, 0, std::ptr::null()) == 0 {
            let e = last_error();
            if e != ERROR_SERVICE_ALREADY_RUNNING {
                return Err(error_text(e));
            }
        }
    }
    Ok(())
}

/// Change a service's start type. Falls back to the Services registry key when the
/// SCM refuses (some services have restrictive SCM ACLs but writable keys).
pub fn set_start(name: &str, start_type: u32, delayed: bool) -> Result<(), String> {
    match open(name, SERVICE_CHANGE_CONFIG) {
        Ok((_m, s)) => unsafe {
            let ok = ChangeServiceConfigW(
                s.0,
                SERVICE_NO_CHANGE,
                start_type,
                SERVICE_NO_CHANGE,
                std::ptr::null(),
                std::ptr::null(),
                std::ptr::null_mut(),
                std::ptr::null(),
                std::ptr::null(),
                std::ptr::null(),
                std::ptr::null(),
            );
            if ok == 0 {
                let e = last_error();
                if e == ERROR_ACCESS_DENIED {
                    return set_start_registry(name, start_type, delayed);
                }
                return Err(error_text(e));
            }
            if start_type == START_AUTO {
                let info = SERVICE_DELAYED_AUTO_START_INFO { fDelayedAutostart: delayed as i32 };
                ChangeServiceConfig2W(
                    s.0,
                    SERVICE_CONFIG_DELAYED_AUTO_START_INFO,
                    &info as *const _ as *const c_void,
                );
            }
            Ok(())
        },
        Err(e) if e == ERROR_ACCESS_DENIED => set_start_registry(name, start_type, delayed),
        Err(e) => Err(error_text(e)),
    }
}

fn set_start_registry(name: &str, start_type: u32, delayed: bool) -> Result<(), String> {
    let path = format!(r"SYSTEM\CurrentControlSet\Services\{name}");
    registry::set_dword(registry::Hive::HKLM, &path, "Start", start_type)?;
    if start_type == START_AUTO {
        registry::set_dword(registry::Hive::HKLM, &path, "DelayedAutostart", delayed as u32)?;
    }
    Ok(())
}
