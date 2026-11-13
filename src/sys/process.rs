//! Process enumeration (one NtQuerySystemInformation call, no per-process handles)
//! and process tuning: priority class, EcoQoS / power throttling, memory and I/O priority.

use super::{ntdll, OwnedHandle};
use std::collections::HashMap;
use std::ffi::c_void;
use std::time::Instant;
use windows_sys::Win32::Foundation::FILETIME;
use windows_sys::Win32::System::Threading::{
    GetPriorityClass, GetProcessInformation, GetProcessTimes, GetSystemTimes, OpenProcess,
    ProcessMemoryPriority, ProcessPowerThrottling, QueryFullProcessImageNameW, SetPriorityClass,
    SetProcessInformation, TerminateProcess, ABOVE_NORMAL_PRIORITY_CLASS,
    BELOW_NORMAL_PRIORITY_CLASS, HIGH_PRIORITY_CLASS, IDLE_PRIORITY_CLASS,
    MEMORY_PRIORITY_INFORMATION, NORMAL_PRIORITY_CLASS, PROCESS_POWER_THROTTLING_STATE,
    PROCESS_QUERY_INFORMATION, PROCESS_QUERY_LIMITED_INFORMATION, PROCESS_SET_INFORMATION,
    PROCESS_SET_QUOTA, PROCESS_TERMINATE,
};

pub const PRIO_IDLE: u32 = IDLE_PRIORITY_CLASS;
pub const PRIO_BELOW_NORMAL: u32 = BELOW_NORMAL_PRIORITY_CLASS;
pub const PRIO_NORMAL: u32 = NORMAL_PRIORITY_CLASS;
pub const PRIO_ABOVE_NORMAL: u32 = ABOVE_NORMAL_PRIORITY_CLASS;
pub const PRIO_HIGH: u32 = HIGH_PRIORITY_CLASS;

/// Power-throttling control bits.
pub const THROTTLE_EXECUTION_SPEED: u32 = 0x1;
pub const THROTTLE_IGNORE_TIMER_RESOLUTION: u32 = 0x4;

pub const MEMORY_PRIORITY_BELOW_NORMAL: u32 = 4;
pub const IO_PRIORITY_LOW: u32 = 1;
pub const IO_PRIORITY_NORMAL: u32 = 2;

/// Priority classes a user can pick in the UI, in display order.
pub const USER_PRIORITIES: [(u32, &str); 5] = [
    (PRIO_HIGH, "High"),
    (PRIO_ABOVE_NORMAL, "Above normal"),
    (PRIO_NORMAL, "Normal"),
    (PRIO_BELOW_NORMAL, "Below normal"),
    (PRIO_IDLE, "Low"),
];

#[derive(Clone, Debug)]
pub struct ProcInfo {
    pub pid: u32,
    pub ppid: u32,
    pub name: String,
    pub name_lc: String,
    pub session: u32,
    /// user + kernel time, 100 ns units
    pub cpu_time: u64,
    pub create_time: i64,
    pub working_set: u64,
    /// Private working set - what Task Manager shows as "Memory".
    pub private_ws: u64,
    pub base_priority: i32,
}

impl ProcInfo {
    pub fn priority_label(&self) -> &'static str {
        match self.base_priority {
            0..=4 => "Low",
            5..=7 => "Below normal",
            8..=9 => "Normal",
            10..=12 => "Above normal",
            13..=23 => "High",
            _ => "Realtime",
        }
    }
}

#[repr(C)]
struct UnicodeString {
    length: u16,
    maximum_length: u16,
    buffer: *mut u16,
}

/// Documented prefix of SYSTEM_PROCESS_INFORMATION (x64 layout).
#[repr(C)]
struct SystemProcessInformation {
    next_entry_offset: u32,
    number_of_threads: u32,
    working_set_private_size: i64,
    hard_fault_count: u32,
    number_of_threads_high_watermark: u32,
    cycle_time: u64,
    create_time: i64,
    user_time: i64,
    kernel_time: i64,
    image_name: UnicodeString,
    base_priority: i32,
    unique_process_id: usize,
    inherited_from_unique_process_id: usize,
    handle_count: u32,
    session_id: u32,
    unique_process_key: usize,
    peak_virtual_size: usize,
    virtual_size: usize,
    page_fault_count: u32,
    peak_working_set_size: usize,
    working_set_size: usize,
    quota_peak_paged_pool_usage: usize,
    quota_paged_pool_usage: usize,
    quota_peak_non_paged_pool_usage: usize,
    quota_non_paged_pool_usage: usize,
    pagefile_usage: usize,
    peak_pagefile_usage: usize,
    private_page_count: usize,
}

/// Reusable process snapshotter. Keeps its buffer between calls so polling allocates nothing big.
pub struct Snapshotter {
    buf: Vec<u64>,
}

impl Default for Snapshotter {
    fn default() -> Self {
        Self::new()
    }
}

impl Snapshotter {
    pub fn new() -> Self {
        Self { buf: vec![0u64; 64 * 1024] } // 512 KB, grows if needed
    }

    pub fn take(&mut self) -> Vec<ProcInfo> {
        let Some(query) = ntdll::get().query_system_information else {
            return Vec::new();
        };
        loop {
            let mut needed = 0u32;
            let bytes = (self.buf.len() * 8) as u32;
            let st = unsafe {
                query(
                    ntdll::SYSTEM_PROCESS_INFORMATION,
                    self.buf.as_mut_ptr() as *mut c_void,
                    bytes,
                    &mut needed,
                )
            };
            if st == ntdll::STATUS_INFO_LENGTH_MISMATCH {
                let want = (needed as usize / 8 + 16 * 1024).max(self.buf.len() * 2);
                self.buf.resize(want, 0);
                continue;
            }
            if st < 0 {
                return Vec::new();
            }
            break;
        }

        let mut out = Vec::with_capacity(400);
        let base = self.buf.as_ptr() as *const u8;
        let mut offset = 0usize;
        loop {
            // SAFETY: the kernel filled `buf` with a chain of SYSTEM_PROCESS_INFORMATION
            // records linked by next_entry_offset; image_name points into the same buffer.
            let p = unsafe { &*(base.add(offset) as *const SystemProcessInformation) };
            let pid = p.unique_process_id as u32;
            let name = if p.image_name.buffer.is_null() || p.image_name.length == 0 {
                if pid == 0 { "Idle".to_string() } else { "System".to_string() }
            } else {
                let s = unsafe {
                    std::slice::from_raw_parts(p.image_name.buffer, p.image_name.length as usize / 2)
                };
                String::from_utf16_lossy(s)
            };
            out.push(ProcInfo {
                pid,
                ppid: p.inherited_from_unique_process_id as u32,
                name_lc: name.to_lowercase(),
                name,
                session: p.session_id,
                cpu_time: (p.user_time + p.kernel_time).max(0) as u64,
                create_time: p.create_time,
                working_set: p.working_set_size as u64,
                private_ws: p.working_set_private_size.max(0) as u64,
                base_priority: p.base_priority,
            });
            if p.next_entry_offset == 0 {
                break;
            }
            offset += p.next_entry_offset as usize;
        }
        out
    }
}

/// Per-process CPU usage from successive snapshots (percent of the whole machine).
pub struct CpuTracker {
    prev: HashMap<u32, (i64, u64)>,
    last: Option<Instant>,
    ncpu: f64,
}

impl Default for CpuTracker {
    fn default() -> Self {
        Self::new()
    }
}

impl CpuTracker {
    pub fn new() -> Self {
        Self { prev: HashMap::new(), last: None, ncpu: logical_cpus() as f64 }
    }

    pub fn update(&mut self, procs: &[ProcInfo]) -> HashMap<u32, f32> {
        let now = Instant::now();
        let elapsed = self.last.map(|l| now.duration_since(l).as_secs_f64()).unwrap_or(0.0);
        let mut out = HashMap::with_capacity(procs.len());
        let mut next = HashMap::with_capacity(procs.len());
        for p in procs {
            if let Some(&(ct, cpu)) = self.prev.get(&p.pid) {
                if ct == p.create_time && elapsed > 0.05 {
                    let secs = p.cpu_time.saturating_sub(cpu) as f64 / 1e7;
                    out.insert(p.pid, ((secs / elapsed / self.ncpu) * 100.0).min(100.0) as f32);
                }
            }
            next.insert(p.pid, (p.create_time, p.cpu_time));
        }
        self.prev = next;
        self.last = Some(now);
        out
    }
}

pub fn logical_cpus() -> usize {
    std::thread::available_parallelism().map(|n| n.get()).unwrap_or(1)
}

fn ft(f: &FILETIME) -> u64 {
    (f.dwHighDateTime as u64) << 32 | f.dwLowDateTime as u64
}

/// Whole-machine CPU usage from GetSystemTimes deltas.
#[derive(Default)]
pub struct SystemCpu {
    prev: Option<(u64, u64, u64)>,
}

impl SystemCpu {
    pub fn sample(&mut self) -> f32 {
        let (mut idle, mut kernel, mut user) = unsafe { std::mem::zeroed::<(FILETIME, FILETIME, FILETIME)>() };
        if unsafe { GetSystemTimes(&mut idle, &mut kernel, &mut user) } == 0 {
            return 0.0;
        }
        let cur = (ft(&idle), ft(&kernel), ft(&user));
        let pct = match self.prev {
            Some((pi, pk, pu)) => {
                let di = cur.0.saturating_sub(pi) as f64;
                let total = (cur.1.saturating_sub(pk) + cur.2.saturating_sub(pu)) as f64;
                if total > 0.0 { ((total - di) / total * 100.0).clamp(0.0, 100.0) as f32 } else { 0.0 }
            }
            None => 0.0,
        };
        self.prev = Some(cur);
        pct
    }
}

/// An open process handle with tuning helpers.
pub struct ProcHandle(OwnedHandle);

impl ProcHandle {
    /// Open for query + set. Falls back to limited query rights.
    pub fn open_rw(pid: u32) -> Option<Self> {
        unsafe {
            let h = OpenProcess(PROCESS_SET_INFORMATION | PROCESS_QUERY_INFORMATION, 0, pid);
            if let Some(h) = OwnedHandle::new(h) {
                return Some(Self(h));
            }
            OwnedHandle::new(OpenProcess(
                PROCESS_SET_INFORMATION | PROCESS_QUERY_LIMITED_INFORMATION,
                0,
                pid,
            ))
            .map(Self)
        }
    }

    pub fn open_query(pid: u32) -> Option<Self> {
        unsafe { OwnedHandle::new(OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, 0, pid)).map(Self) }
    }

    pub fn priority_class(&self) -> Option<u32> {
        let c = unsafe { GetPriorityClass(self.0 .0) };
        (c != 0).then_some(c)
    }

    pub fn set_priority_class(&self, class: u32) -> bool {
        unsafe { SetPriorityClass(self.0 .0, class) != 0 }
    }

    /// (control_mask, state_mask)
    pub fn power_throttling(&self) -> Option<(u32, u32)> {
        let mut s = PROCESS_POWER_THROTTLING_STATE { Version: 1, ControlMask: 0, StateMask: 0 };
        let ok = unsafe {
            GetProcessInformation(
                self.0 .0,
                ProcessPowerThrottling,
                &mut s as *mut _ as *mut c_void,
                std::mem::size_of::<PROCESS_POWER_THROTTLING_STATE>() as u32,
            )
        };
        (ok != 0).then_some((s.ControlMask, s.StateMask))
    }

    pub fn set_power_throttling(&self, control: u32, state: u32) -> bool {
        let s = PROCESS_POWER_THROTTLING_STATE { Version: 1, ControlMask: control, StateMask: state };
        unsafe {
            SetProcessInformation(
                self.0 .0,
                ProcessPowerThrottling,
                &s as *const _ as *const c_void,
                std::mem::size_of::<PROCESS_POWER_THROTTLING_STATE>() as u32,
            ) != 0
        }
    }

    /// EcoQoS ("Efficiency mode" in Task Manager).
    pub fn set_efficiency_mode(&self, on: bool) -> bool {
        if on {
            self.set_power_throttling(THROTTLE_EXECUTION_SPEED, THROTTLE_EXECUTION_SPEED)
        } else {
            self.set_power_throttling(0, 0)
        }
    }

    pub fn memory_priority(&self) -> Option<u32> {
        let mut m = MEMORY_PRIORITY_INFORMATION { MemoryPriority: 0 };
        let ok = unsafe {
            GetProcessInformation(
                self.0 .0,
                ProcessMemoryPriority,
                &mut m as *mut _ as *mut c_void,
                std::mem::size_of::<MEMORY_PRIORITY_INFORMATION>() as u32,
            )
        };
        (ok != 0).then_some(m.MemoryPriority)
    }

    pub fn set_memory_priority(&self, p: u32) -> bool {
        let m = MEMORY_PRIORITY_INFORMATION { MemoryPriority: p };
        unsafe {
            SetProcessInformation(
                self.0 .0,
                ProcessMemoryPriority,
                &m as *const _ as *const c_void,
                std::mem::size_of::<MEMORY_PRIORITY_INFORMATION>() as u32,
            ) != 0
        }
    }

    pub fn io_priority(&self) -> Option<u32> {
        let f = ntdll::get().query_information_process?;
        let mut v = 0u32;
        let mut ret = 0u32;
        let st = unsafe {
            f(self.0 .0, ntdll::PROCESS_IO_PRIORITY, &mut v as *mut _ as *mut c_void, 4, &mut ret)
        };
        (st >= 0).then_some(v)
    }

    pub fn set_io_priority(&self, p: u32) -> bool {
        let Some(f) = ntdll::get().set_information_process else { return false };
        unsafe { f(self.0 .0, ntdll::PROCESS_IO_PRIORITY, &p as *const _ as *const c_void, 4) >= 0 }
    }

    pub fn create_time(&self) -> Option<i64> {
        let mut t: [FILETIME; 4] = unsafe { std::mem::zeroed() };
        let [a, b, c, d] = &mut t;
        let ok = unsafe { GetProcessTimes(self.0 .0, a, b, c, d) };
        (ok != 0).then(|| ft(&t[0]) as i64)
    }
}

pub fn image_path(pid: u32) -> Option<String> {
    let h = ProcHandle::open_query(pid)?;
    let mut buf = [0u16; 1024];
    let mut len = buf.len() as u32;
    let ok = unsafe { QueryFullProcessImageNameW(h.0 .0, 0, buf.as_mut_ptr(), &mut len) };
    (ok != 0).then(|| String::from_utf16_lossy(&buf[..len as usize]))
}

/// Ask Windows to move a process's memory out to the page file / standby list. It pages back in
/// on demand, so this only costs that app a little speed the next time it's used.
pub fn trim_working_set(pid: u32) -> bool {
    use windows_sys::Win32::System::ProcessStatus::K32EmptyWorkingSet;
    unsafe {
        match OwnedHandle::new(OpenProcess(PROCESS_SET_QUOTA | PROCESS_QUERY_LIMITED_INFORMATION, 0, pid)) {
            Some(h) => K32EmptyWorkingSet(h.0) != 0,
            None => false,
        }
    }
}

pub fn kill(pid: u32) -> bool {
    unsafe {
        match OwnedHandle::new(OpenProcess(PROCESS_TERMINATE, 0, pid)) {
            Some(h) => TerminateProcess(h.0, 1) != 0,
            None => false,
        }
    }
}

pub fn set_priority(pid: u32, class: u32) -> bool {
    ProcHandle::open_rw(pid).is_some_and(|h| h.set_priority_class(class))
}

pub fn set_efficiency(pid: u32, on: bool) -> bool {
    ProcHandle::open_rw(pid).is_some_and(|h| h.set_efficiency_mode(on))
}

/// Own process working set + CPU time (to show how light the engine is).
pub fn own_usage() -> (u64, u64) {
    use windows_sys::Win32::System::ProcessStatus::{K32GetProcessMemoryInfo, PROCESS_MEMORY_COUNTERS};
    use windows_sys::Win32::System::Threading::GetCurrentProcess;
    unsafe {
        let mut pmc: PROCESS_MEMORY_COUNTERS = std::mem::zeroed();
        pmc.cb = std::mem::size_of::<PROCESS_MEMORY_COUNTERS>() as u32;
        K32GetProcessMemoryInfo(GetCurrentProcess(), &mut pmc, pmc.cb);
        let mut t: [FILETIME; 4] = std::mem::zeroed();
        let [a, b, c, d] = &mut t;
        GetProcessTimes(GetCurrentProcess(), a, b, c, d);
        (pmc.WorkingSetSize as u64, ft(&t[2]) + ft(&t[3]))
    }
}
