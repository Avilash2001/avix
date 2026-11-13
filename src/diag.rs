//! `avix --diag`: read-only self-test. Prints what Avix can see (no changes are made).

use crate::sys::{self, display, gpu, memory, net, ntdll, power, process, services};
use crate::tweaks::{self, catalog};
use std::fmt::Write as _;

pub fn run() -> String {
    let mut o = String::new();
    let _ = writeln!(o, "Avix {} diagnostics", crate::VERSION);
    let _ = writeln!(o, "elevated: {}", sys::elevation::is_elevated());

    let mut snap = process::Snapshotter::new();
    let t = std::time::Instant::now();
    let procs = snap.take();
    let _ = writeln!(o, "processes: {} (snapshot {:?})", procs.len(), t.elapsed());
    let own = procs.iter().find(|p| p.pid == std::process::id());
    let _ = writeln!(o, "own: {:?}", own.map(|p| (&p.name, p.session, p.base_priority)));

    let (mut matched, mut checked) = (0, 0);
    let session = own.map(|p| p.session).unwrap_or(1);
    for p in procs.iter().filter(|p| p.pid > 4 && p.session == session).take(60) {
        if let Some(h) = process::ProcHandle::open_query(p.pid) {
            checked += 1;
            if h.create_time() == Some(p.create_time) {
                matched += 1;
            }
        }
    }
    let _ = writeln!(o, "create_time snapshot==handle: {matched}/{checked}");
    let mut by_cpu: Vec<_> = procs.iter().filter(|p| p.pid > 0).collect();
    by_cpu.sort_by_key(|p| std::cmp::Reverse(p.private_ws));
    for p in by_cpu.iter().take(5) {
        let _ = writeln!(o, "  top mem: {} pid={} {} prio={}", p.name, p.pid, sys::fmt_bytes(p.private_ws), p.priority_label());
    }

    let known = sys::games::windows_known();
    let _ = writeln!(o, "games Windows recognises: {}", known.len());
    for g in &known {
        let running = procs.iter().any(|p| p.name_lc == g.name_lc);
        let _ = writeln!(o, "  game {}{}", g.path_lc, if running { "  [running]" } else { "" });
    }
    let _ = writeln!(o, "power active: {:?}", power::active().map(|g| power::name_of(&g)));
    for (g, n) in power::list() {
        let _ = writeln!(o, "  plan {} {}", power::to_string(&g), n);
    }
    let m = memory::status();
    let _ = writeln!(o, "memory: {} / {} load {}%", sys::fmt_bytes(m.total - m.avail), sys::fmt_bytes(m.total), m.load);
    let _ = writeln!(o, "standby/free: {:?}", memory::lists().map(|(s, f)| (sys::fmt_bytes(s), sys::fmt_bytes(f))));
    let _ = writeln!(o, "timer resolution (min,max,cur): {:?}", ntdll::timer_resolution());
    for s in ["wuauserv", "DoSvc", "BITS", "DiagTrack", "SysMain"] {
        let _ = writeln!(o, "service {s}: {:?}", services::query(s));
    }
    for d in display::displays() {
        let _ = writeln!(o, "display {} '{}' {}x{} {}Hz (max {}Hz)", d.device, d.monitor, d.width, d.height, d.current_hz, d.max_hz);
    }
    for a in net::active_adapters() {
        let _ = writeln!(o, "adapter '{}' wifi={} {} Mbps", a.name, a.wifi, a.speed_mbps);
    }
    if let Some(p) = net::Pinger::new("1.1.1.1") {
        let _ = writeln!(o, "ping 1.1.1.1: {:?} ms", p.ping(1000));
    }
    if let Some(mut g) = gpu::GpuMonitor::new() {
        std::thread::sleep(std::time::Duration::from_millis(500));
        let s = g.sample();
        let _ = writeln!(o, "gpu: {:.1}% vram {} ({} procs)", s.utilization, sys::fmt_bytes(s.dedicated_used), s.per_pid.len());
    }
    let t = std::time::Instant::now();
    for tw in catalog::all() {
        let _ = writeln!(o, "tweak {:<32} {:?}", tw.id, tweaks::state(tw));
    }
    let _ = writeln!(o, "(tweak states took {:?})", t.elapsed());
    let st = tweaks::startup::list();
    for s in &st {
        let _ = writeln!(o, "startup {:<28} enabled={} {:?}", s.name, s.enabled, s.source);
    }
    let t = std::time::Instant::now();
    match tweaks::apps::list_installed() {
        Ok(pkgs) => {
            let _ = writeln!(o, "appx packages: {} (listed in {:?})", pkgs.len(), t.elapsed());
            for b in tweaks::apps::BLOAT {
                if pkgs.iter().any(|p| tweaks::apps::glob_match(b.pattern, &p.name)) {
                    let _ = writeln!(o, "  bloat installed: {} ({}) recommended={}", b.label, b.pattern, b.recommended);
                }
            }
        }
        Err(e) => {
            let _ = writeln!(o, "appx list failed: {e}");
        }
    }
    let _ = writeln!(o, "onedrive uninstall string: {:?}", tweaks::apps::onedrive_uninstall_string().is_some());
    let _ = writeln!(o, "autostart task: {}", crate::autostart::is_enabled());
    let _ = writeln!(o, "valorant launch priority: {:?}", tweaks::launch_priority("VALORANT-Win64-Shipping.exe"));
    o
}
