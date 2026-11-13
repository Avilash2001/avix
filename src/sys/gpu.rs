//! GPU utilisation (overall + per process) and dedicated VRAM use via the same
//! "GPU Engine" performance counters Task Manager reads. Used only by the dashboard.

use super::wide;
use std::collections::HashMap;
use windows_sys::Win32::System::Performance::{
    PdhAddEnglishCounterW, PdhCloseQuery, PdhCollectQueryData, PdhGetFormattedCounterArrayW,
    PdhOpenQueryW, PDH_FMT_COUNTERVALUE_ITEM_W, PDH_FMT_DOUBLE, PDH_MORE_DATA,
};

pub struct GpuMonitor {
    query: isize,
    engines: isize,
    memory: isize,
}

unsafe impl Send for GpuMonitor {}

#[derive(Clone, Debug, Default)]
pub struct GpuSample {
    pub utilization: f32,
    pub per_pid: HashMap<u32, f32>,
    pub dedicated_used: u64,
}

impl GpuMonitor {
    pub fn new() -> Option<Self> {
        unsafe {
            let mut q = 0isize;
            if PdhOpenQueryW(std::ptr::null(), 0, &mut q) != 0 {
                return None;
            }
            let mut e = 0isize;
            let mut m = 0isize;
            let p1 = wide(r"\GPU Engine(*)\Utilization Percentage");
            let p2 = wide(r"\GPU Adapter Memory(*)\Dedicated Usage");
            if PdhAddEnglishCounterW(q, p1.as_ptr(), 0, &mut e) != 0 {
                PdhCloseQuery(q);
                return None;
            }
            PdhAddEnglishCounterW(q, p2.as_ptr(), 0, &mut m);
            PdhCollectQueryData(q); // prime rate counters
            Some(Self { query: q, engines: e, memory: m })
        }
    }

    fn read_array(counter: isize) -> Vec<(String, f64)> {
        if counter == 0 {
            return Vec::new();
        }
        unsafe {
            let mut size = 0u32;
            let mut count = 0u32;
            let rc = PdhGetFormattedCounterArrayW(counter, PDH_FMT_DOUBLE, &mut size, &mut count, std::ptr::null_mut());
            if rc != PDH_MORE_DATA || size == 0 {
                return Vec::new();
            }
            let mut buf = vec![0u64; size as usize / 8 + 1];
            let items = buf.as_mut_ptr() as *mut PDH_FMT_COUNTERVALUE_ITEM_W;
            if PdhGetFormattedCounterArrayW(counter, PDH_FMT_DOUBLE, &mut size, &mut count, items) != 0 {
                return Vec::new();
            }
            (0..count as usize)
                .filter_map(|i| {
                    let it = &*items.add(i);
                    if it.FmtValue.CStatus != 0 {
                        return None;
                    }
                    Some((super::from_wide_ptr(it.szName), it.FmtValue.Anonymous.doubleValue))
                })
                .collect()
        }
    }

    pub fn sample(&mut self) -> GpuSample {
        unsafe {
            PdhCollectQueryData(self.query);
        }
        // instance: pid_1234_luid_0x..._0x..._phys_0_eng_3_engtype_3D
        let mut per_engine: HashMap<String, f64> = HashMap::new();
        let mut per_pid: HashMap<u32, f32> = HashMap::new();
        for (name, v) in Self::read_array(self.engines) {
            let Some(rest) = name.strip_prefix("pid_") else { continue };
            let Some((pid, engine)) = rest.split_once('_') else { continue };
            let pid: u32 = pid.parse().unwrap_or(0);
            *per_engine.entry(engine.to_string()).or_default() += v;
            let e = per_pid.entry(pid).or_default();
            *e = e.max(v as f32);
        }
        let utilization = per_engine.values().cloned().fold(0.0, f64::max).min(100.0) as f32;
        let dedicated_used = Self::read_array(self.memory).iter().map(|(_, v)| *v as u64).sum();
        GpuSample { utilization, per_pid, dedicated_used }
    }
}

impl Drop for GpuMonitor {
    fn drop(&mut self) {
        unsafe {
            PdhCloseQuery(self.query);
        }
    }
}
