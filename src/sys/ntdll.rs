//! Native API entry points resolved at runtime from ntdll.dll.
//! These are stable, long-standing system calls used by Task Manager, RAMMap,
//! Process Explorer and friends; resolving them dynamically keeps linking simple.

use std::ffi::c_void;
use std::sync::OnceLock;
use windows_sys::Win32::Foundation::HANDLE;
use windows_sys::Win32::System::LibraryLoader::{GetModuleHandleW, GetProcAddress};

pub type NtQuerySystemInformation = unsafe extern "system" fn(u32, *mut c_void, u32, *mut u32) -> i32;
pub type NtSetSystemInformation = unsafe extern "system" fn(u32, *const c_void, u32) -> i32;
pub type NtSetTimerResolution = unsafe extern "system" fn(u32, u8, *mut u32) -> i32;
pub type NtQueryTimerResolution = unsafe extern "system" fn(*mut u32, *mut u32, *mut u32) -> i32;
pub type NtSetInformationProcess = unsafe extern "system" fn(HANDLE, u32, *const c_void, u32) -> i32;
pub type NtQueryInformationProcess =
    unsafe extern "system" fn(HANDLE, u32, *mut c_void, u32, *mut u32) -> i32;

pub const STATUS_INFO_LENGTH_MISMATCH: i32 = 0xC000_0004u32 as i32;
pub const SYSTEM_PROCESS_INFORMATION: u32 = 5;
pub const SYSTEM_MEMORY_LIST_INFORMATION: u32 = 80;
pub const PROCESS_IO_PRIORITY: u32 = 33;

pub struct Ntdll {
    pub query_system_information: Option<NtQuerySystemInformation>,
    pub set_system_information: Option<NtSetSystemInformation>,
    pub set_timer_resolution: Option<NtSetTimerResolution>,
    pub query_timer_resolution: Option<NtQueryTimerResolution>,
    pub set_information_process: Option<NtSetInformationProcess>,
    pub query_information_process: Option<NtQueryInformationProcess>,
}

unsafe fn resolve<T: Copy>(module: *mut c_void, name: &[u8]) -> Option<T> {
    let f = GetProcAddress(module, name.as_ptr())?;
    Some(std::mem::transmute_copy::<_, T>(&f))
}

pub fn get() -> &'static Ntdll {
    static N: OnceLock<Ntdll> = OnceLock::new();
    N.get_or_init(|| unsafe {
        let name = super::wide("ntdll.dll");
        let m = GetModuleHandleW(name.as_ptr());
        if m.is_null() {
            return Ntdll {
                query_system_information: None,
                set_system_information: None,
                set_timer_resolution: None,
                query_timer_resolution: None,
                set_information_process: None,
                query_information_process: None,
            };
        }
        Ntdll {
            query_system_information: resolve(m, b"NtQuerySystemInformation\0"),
            set_system_information: resolve(m, b"NtSetSystemInformation\0"),
            set_timer_resolution: resolve(m, b"NtSetTimerResolution\0"),
            query_timer_resolution: resolve(m, b"NtQueryTimerResolution\0"),
            set_information_process: resolve(m, b"NtSetInformationProcess\0"),
            query_information_process: resolve(m, b"NtQueryInformationProcess\0"),
        }
    })
}

/// (min, max, current) timer resolution in 100 ns units. Note "max" = finest.
pub fn timer_resolution() -> Option<(u32, u32, u32)> {
    let f = get().query_timer_resolution?;
    let (mut min, mut max, mut cur) = (0u32, 0u32, 0u32);
    let st = unsafe { f(&mut min, &mut max, &mut cur) };
    (st >= 0).then_some((min, max, cur))
}

/// Request (or release) the finest timer resolution the system supports.
pub fn request_timer_resolution(enable: bool) -> bool {
    let (Some(set), Some((_, finest, _))) = (get().set_timer_resolution, timer_resolution()) else {
        return false;
    };
    if enable {
        // Windows 11 ignores timer requests from processes whose windows are hidden
        // (our tray window) unless the process opts out of that power-saving behaviour.
        use windows_sys::Win32::System::Threading::{
            GetCurrentProcess, ProcessPowerThrottling, SetProcessInformation, PROCESS_POWER_THROTTLING_STATE,
        };
        let s = PROCESS_POWER_THROTTLING_STATE {
            Version: 1,
            ControlMask: super::process::THROTTLE_IGNORE_TIMER_RESOLUTION,
            StateMask: 0,
        };
        unsafe {
            SetProcessInformation(
                GetCurrentProcess(),
                ProcessPowerThrottling,
                &s as *const _ as *const c_void,
                std::mem::size_of::<PROCESS_POWER_THROTTLING_STATE>() as u32,
            );
        }
    }
    let mut cur = 0u32;
    unsafe { set(finest, enable as u8, &mut cur) >= 0 }
}
