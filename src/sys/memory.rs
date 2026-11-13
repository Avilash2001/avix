//! Memory status, standby-list inspection and purge (same mechanism RAMMap / ISLC use).

use super::{ntdll, wide};
use std::ffi::c_void;
use windows_sys::Win32::Foundation::{CloseHandle, GetLastError, ERROR_SUCCESS, HANDLE, LUID};
use windows_sys::Win32::Security::{
    AdjustTokenPrivileges, LookupPrivilegeValueW, LUID_AND_ATTRIBUTES, SE_PRIVILEGE_ENABLED,
    TOKEN_ADJUST_PRIVILEGES, TOKEN_PRIVILEGES, TOKEN_QUERY,
};
use windows_sys::Win32::System::SystemInformation::{GlobalMemoryStatusEx, MEMORYSTATUSEX};
use windows_sys::Win32::System::Threading::{GetCurrentProcess, OpenProcessToken};

#[derive(Clone, Copy, Debug, Default)]
pub struct MemStatus {
    pub total: u64,
    pub avail: u64,
    pub load: u32,
    pub commit_limit: u64,
}

pub fn status() -> MemStatus {
    unsafe {
        let mut m: MEMORYSTATUSEX = std::mem::zeroed();
        m.dwLength = std::mem::size_of::<MEMORYSTATUSEX>() as u32;
        if GlobalMemoryStatusEx(&mut m) == 0 {
            return MemStatus::default();
        }
        MemStatus {
            total: m.ullTotalPhys,
            avail: m.ullAvailPhys,
            load: m.dwMemoryLoad,
            commit_limit: m.ullTotalPageFile,
        }
    }
}

#[repr(C)]
struct SystemMemoryListInformation {
    zero_page_count: usize,
    free_page_count: usize,
    modified_page_count: usize,
    modified_no_write_page_count: usize,
    bad_page_count: usize,
    page_count_by_priority: [usize; 8],
    repurposed_pages_by_priority: [usize; 8],
    modified_page_count_page_file: usize,
}

const PAGE: u64 = 4096;
const MEMORY_PURGE_STANDBY_LIST: u32 = 4;

pub fn enable_privilege(name: &str) -> bool {
    unsafe {
        let mut token: HANDLE = std::ptr::null_mut();
        if OpenProcessToken(GetCurrentProcess(), TOKEN_ADJUST_PRIVILEGES | TOKEN_QUERY, &mut token) == 0 {
            return false;
        }
        let mut luid = LUID { LowPart: 0, HighPart: 0 };
        let n = wide(name);
        let mut ok = LookupPrivilegeValueW(std::ptr::null(), n.as_ptr(), &mut luid) != 0;
        if ok {
            let tp = TOKEN_PRIVILEGES {
                PrivilegeCount: 1,
                Privileges: [LUID_AND_ATTRIBUTES { Luid: luid, Attributes: SE_PRIVILEGE_ENABLED }],
            };
            AdjustTokenPrivileges(token, 0, &tp, 0, std::ptr::null_mut(), std::ptr::null_mut());
            ok = GetLastError() == ERROR_SUCCESS;
        }
        CloseHandle(token);
        ok
    }
}

/// (standby bytes, free+zero bytes). Needs admin (SeProfileSingleProcessPrivilege).
pub fn lists() -> Option<(u64, u64)> {
    let f = ntdll::get().query_system_information?;
    enable_privilege("SeProfileSingleProcessPrivilege");
    let mut info: SystemMemoryListInformation = unsafe { std::mem::zeroed() };
    let mut ret = 0u32;
    let st = unsafe {
        f(
            ntdll::SYSTEM_MEMORY_LIST_INFORMATION,
            &mut info as *mut _ as *mut c_void,
            std::mem::size_of::<SystemMemoryListInformation>() as u32,
            &mut ret,
        )
    };
    if st < 0 {
        return None;
    }
    let standby: u64 = info.page_count_by_priority.iter().map(|&p| p as u64).sum::<u64>() * PAGE;
    let free = (info.free_page_count as u64 + info.zero_page_count as u64) * PAGE;
    Some((standby, free))
}

/// Empty the standby list (cached file pages). Safe: Windows simply re-reads from disk if needed.
pub fn purge_standby() -> Result<(), String> {
    let f = ntdll::get().set_system_information.ok_or("NtSetSystemInformation unavailable")?;
    if !enable_privilege("SeProfileSingleProcessPrivilege") {
        return Err("needs administrator rights".into());
    }
    let cmd = MEMORY_PURGE_STANDBY_LIST;
    let st = unsafe {
        f(ntdll::SYSTEM_MEMORY_LIST_INFORMATION, &cmd as *const _ as *const c_void, 4)
    };
    if st >= 0 {
        Ok(())
    } else {
        Err(format!("NTSTATUS 0x{:08X}", st as u32))
    }
}
