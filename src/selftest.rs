//! `avix --selftest` (debug builds only): admin integration test.
//! Every system-changing feature is exercised as a round trip and the PC is checked
//! to be byte-for-byte back where it started. Tweaks that are already applied are
//! skipped so the user's own settings are never touched.

use crate::sys::registry::{self, Hive, RegValue};
use crate::sys::{self, memory, ntdll, power, process, services};
use crate::tweaks::{self, apps, catalog, startup, tasks, Action, TweakState, TCP_INTERFACES};
use std::fmt::Write as _;
use std::time::{Duration, Instant};

struct Report {
    out: String,
    pass: u32,
    fail: u32,
}

impl Report {
    fn check(&mut self, name: &str, ok: bool, detail: impl AsRef<str>) {
        if ok {
            self.pass += 1;
        } else {
            self.fail += 1;
        }
        let _ = writeln!(self.out, "[{}] {name} {}", if ok { "PASS" } else { "FAIL" }, detail.as_ref());
    }
    fn note(&mut self, s: impl AsRef<str>) {
        let _ = writeln!(self.out, "       {}", s.as_ref());
    }
    fn section(&mut self, s: &str) {
        let _ = writeln!(self.out, "\n== {s} ==");
    }
}

pub fn run() -> String {
    let mut r = Report { out: String::new(), pass: 0, fail: 0 };
    let _ = writeln!(r.out, "Avix {} admin self-test - {}", crate::VERSION, sys::local_datetime());
    r.check("running elevated", sys::elevation::is_elevated(), "");
    memory::enable_privilege("SeDebugPrivilege");

    // AVIX_SELFTEST_ONLY=timer,apps,...  runs a subset.
    let only = std::env::var("AVIX_SELFTEST_ONLY").ok();
    let want = |s: &str| only.as_deref().map_or(true, |o| o.split(',').any(|x| x.trim() == s));
    let sections: [(&str, fn(&mut Report)); 10] = [
        ("restore", restore_point),
        ("power", power_plans),
        ("standby", standby),
        ("timer", timer),
        ("process", process_ops),
        ("ifeo", launch_priority),
        ("startup", startup_items),
        ("autostart", autostart),
        ("apps", app_removal),
        ("tweaks", tweak_round_trips),
    ];
    for (name, f) in sections {
        if want(name) {
            f(&mut r);
        }
    }

    if want("health") {
        r.section("health scan");
        let t = Instant::now();
        let f = crate::health::scan();
        r.check("health scan runs", !f.is_empty(), format!("{} findings in {:?}", f.len(), t.elapsed()));
        for x in &f {
            r.note(format!("{:?}: {}", x.severity, x.title));
        }
    }

    let _ = writeln!(r.out, "\nSUMMARY: {} passed, {} failed", r.pass, r.fail);
    r.out
}

fn restore_point(r: &mut Report) {
    r.section("system restore point");
    let t = Instant::now();
    match tweaks::create_restore_point() {
        Ok(()) => r.check("create restore point", true, format!("({:?})", t.elapsed())),
        Err(e) => r.check("create restore point", false, e),
    }
}

fn power_plans(r: &mut Report) {
    r.section("power plans");
    let before = power::active();
    r.note(format!("active before: {:?}", before.map(|g| power::name_of(&g))));
    let had_plan = power::list().iter().any(|(_, n)| n == power::AVIX_PLAN_NAME);
    match power::ensure_avix_plan() {
        Ok(g) => {
            r.check("create Avix plan", power::name_of(&g) == power::AVIX_PLAN_NAME, power::to_string(&g));
            let again = power::ensure_avix_plan().ok();
            r.check("ensure is idempotent (no duplicates)", again.is_some_and(|a| power::same(&a, &g)), "");
            let usb = power::read_ac(&g, 0x2a737441_1930_4402_8d77_b2bebba308a3, 0x48e6b7a6_50f5_4782_a5d4_53bb8f07e226);
            let minproc = power::read_ac(&g, 0x54533251_82be_4824_96c1_47b60b740d00, 0x893dee8e_2bef_41e0_89c6_b55d0929964c);
            let parking = power::read_ac(&g, 0x54533251_82be_4824_96c1_47b60b740d00, 0x0cc5b647_c1df_4637_891a_dec35c318583);
            r.check("tuned: USB selective suspend off", usb == Some(0), format!("{usb:?}"));
            r.check("tuned: min processor state 100%", minproc == Some(100), format!("{minproc:?}"));
            r.check("tuned: core parking off", parking == Some(100), format!("{parking:?}"));
            r.check("switch to Avix plan", power::set_active(&g).is_ok() && power::active().is_some_and(|a| power::same(&a, &g)), "");
            if let Some(b) = before {
                r.check("switch back", power::set_active(&b).is_ok() && power::active().is_some_and(|a| power::same(&a, &b)), "");
            }
            if !had_plan {
                let removed = power::remove_avix_plan();
                r.check(
                    "remove Avix plan",
                    matches!(removed, Ok(true)) && !power::list().iter().any(|(_, n)| n == power::AVIX_PLAN_NAME),
                    format!("{removed:?}"),
                );
            }
        }
        Err(e) => r.check("create Avix plan", false, e),
    }
    r.check("active plan unchanged at end", power::active().map(|g| power::to_u128(&g)) == before.map(|g| power::to_u128(&g)), "");
}

fn standby(r: &mut Report) {
    r.section("standby list");
    let before = memory::lists();
    let res = memory::purge_standby();
    std::thread::sleep(Duration::from_millis(300));
    let after = memory::lists();
    r.check("purge standby list", res.is_ok(), format!("{res:?}"));
    if let (Some((s0, f0)), Some((s1, f1))) = (before, after) {
        r.check(
            "standby shrank / free grew",
            s1 < s0 || f1 > f0,
            format!("standby {} -> {}, free {} -> {}", sys::fmt_bytes(s0), sys::fmt_bytes(s1), sys::fmt_bytes(f0), sys::fmt_bytes(f1)),
        );
    }
}

fn timer(r: &mut Report) {
    r.section("timer resolution");
    let before = ntdll::timer_resolution();
    let on = ntdll::request_timer_resolution(true);
    std::thread::sleep(Duration::from_millis(300));
    let during = ntdll::timer_resolution();
    let off = ntdll::request_timer_resolution(false);
    std::thread::sleep(Duration::from_millis(300));
    let after = ntdll::timer_resolution();
    r.check(
        "request finest resolution",
        on && during.is_some_and(|(_, max, cur)| cur == max),
        format!("before {before:?} during {during:?}"),
    );
    r.check("release request", off, format!("after {after:?}"));
}

fn process_ops(r: &mut Report) {
    r.section("process tuning (on a throwaway ping.exe)");
    let Ok(mut child) = sys::hidden_cmd(&sys::system32("PING.EXE")).args(["-n", "20", "127.0.0.1"]).spawn() else {
        r.check("spawn test process", false, "");
        return;
    };
    let pid = child.id();
    std::thread::sleep(Duration::from_millis(200));
    match process::ProcHandle::open_rw(pid) {
        Some(h) => {
            let c0 = h.priority_class();
            r.check("priority -> High", h.set_priority_class(process::PRIO_HIGH) && h.priority_class() == Some(process::PRIO_HIGH), "");
            r.check("priority -> Below normal", h.set_priority_class(process::PRIO_BELOW_NORMAL) && h.priority_class() == Some(process::PRIO_BELOW_NORMAL), "");
            r.check("EcoQoS on", h.set_efficiency_mode(true) && h.power_throttling() == Some((1, 1)), format!("{:?}", h.power_throttling()));
            r.check("EcoQoS off (high QoS)", h.set_power_throttling(5, 0) && h.power_throttling() == Some((5, 0)), format!("{:?}", h.power_throttling()));
            let m0 = h.memory_priority();
            r.check("memory priority -> below normal", h.set_memory_priority(4) && h.memory_priority() == Some(4), format!("was {m0:?}"));
            let io0 = h.io_priority();
            r.check("I/O priority -> low", h.set_io_priority(1) && h.io_priority() == Some(1), format!("was {io0:?}"));
            r.check("I/O priority -> normal", h.set_io_priority(2) && h.io_priority() == Some(2), "");
            if let Some(c) = c0 {
                h.set_priority_class(c);
            }
        }
        None => r.check("open test process", false, sys::error_text(sys::last_error())),
    }
    let _ = child.kill();
    let _ = child.wait();
}

fn launch_priority(r: &mut Report) {
    r.section("launch priority (IFEO)");
    let exe = "avix-selftest-dummy.exe";
    let key = format!(r"SOFTWARE\Microsoft\Windows NT\CurrentVersion\Image File Execution Options\{exe}");
    let set = tweaks::set_launch_priority(exe, Some(3));
    r.check("set High", set.is_ok() && tweaks::launch_priority(exe) == Some(3), format!("{set:?}"));
    let clear = tweaks::set_launch_priority(exe, None);
    r.check("clear", clear.is_ok() && tweaks::launch_priority(exe).is_none(), format!("{clear:?}"));
    r.check("empty IFEO keys cleaned up", !registry::key_exists(Hive::HKLM, &key), "");
}

fn startup_items(r: &mut Report) {
    r.section("startup apps");
    let list = startup::list();
    r.note(format!("{} entries", list.len()));
    for machine in [false, true] {
        let pick = list.iter().find(|i| {
            !i.enabled && (machine != matches!(i.source, startup::Source::RunUser | startup::Source::FolderUser))
        });
        let Some(item) = pick else {
            r.note(format!("no disabled {} entry to test with", if machine { "all-users" } else { "per-user" }));
            continue;
        };
        let (hive, path) = match item.source {
            startup::Source::RunUser => (Hive::HKCU, r"Software\Microsoft\Windows\CurrentVersion\Explorer\StartupApproved\Run"),
            startup::Source::FolderUser => (Hive::HKCU, r"Software\Microsoft\Windows\CurrentVersion\Explorer\StartupApproved\StartupFolder"),
            startup::Source::RunMachine => (Hive::HKLM, r"Software\Microsoft\Windows\CurrentVersion\Explorer\StartupApproved\Run"),
            startup::Source::RunMachine32 => (Hive::HKLM, r"Software\Microsoft\Windows\CurrentVersion\Explorer\StartupApproved\Run32"),
            startup::Source::FolderMachine => (Hive::HKLM, r"Software\Microsoft\Windows\CurrentVersion\Explorer\StartupApproved\StartupFolder"),
        };
        let raw = registry::get(hive, path, &item.name);
        let e = startup::set_enabled(item, true);
        let now_on = startup::list().iter().any(|i| i.name == item.name && i.source == item.source && i.enabled);
        let d = startup::set_enabled(item, false);
        let now_off = startup::list().iter().any(|i| i.name == item.name && i.source == item.source && !i.enabled);
        // put the exact original bytes back (incl. Task Manager's timestamp)
        let _ = match &raw {
            Some(v) => registry::set(hive, path, &item.name, v),
            None => registry::delete(hive, path, &item.name),
        };
        let exact = registry::get(hive, path, &item.name) == raw;
        r.check(
            &format!("toggle '{}' ({})", item.name, item.source.label()),
            e.is_ok() && now_on && d.is_ok() && now_off && exact,
            format!("enable {e:?} -> on={now_on}, disable {d:?} -> off={now_off}, original restored={exact}"),
        );
    }
}

fn autostart(r: &mut Report) {
    r.section("start with Windows (scheduled task)");
    if crate::autostart::is_enabled() {
        r.note("already enabled by the user - skipped so it isn't removed");
        return;
    }
    let en = crate::autostart::enable();
    let on = crate::autostart::is_enabled();
    let xml = sys::hidden_cmd(&sys::system32("schtasks.exe"))
        .args(["/Query", "/TN", "Avix", "/XML"])
        .output()
        .map(|o| String::from_utf8_lossy(&o.stdout).to_string())
        .unwrap_or_default();
    r.check("create logon task", en.is_ok() && on, format!("{en:?}"));
    r.check("task runs elevated, no time limit, --tray", xml.contains("HighestAvailable") && xml.contains("PT0S") && xml.contains("--tray"), "");
    let dis = crate::autostart::disable();
    r.check("remove logon task", dis.is_ok() && !crate::autostart::is_enabled(), format!("{dis:?}"));
}

fn app_removal(r: &mut Report) {
    r.section("Store app removal (no-op on a fake package)");
    let t = Instant::now();
    let list = apps::list_installed();
    r.check("list packages (all users)", list.as_ref().is_ok_and(|v| !v.is_empty()), format!("{} in {:?}", list.as_ref().map(|v| v.len()).unwrap_or(0), t.elapsed()));
    let t = Instant::now();
    let res = apps::remove(&["Avix.SelfTest.DoesNotExist".to_string()]);
    r.check(
        "removal script runs elevated",
        res.as_ref().is_ok_and(|v| v.len() == 1 && v[0].ok),
        format!("{:?} in {:?}", res.as_ref().map(|v| v.iter().map(|x| (x.name.clone(), x.ok)).collect::<Vec<_>>()), t.elapsed()),
    );
    if let Ok(v) = &res {
        for x in v.iter().filter(|x| !x.error.is_empty()) {
            r.note(format!("note: {}", x.error));
        }
    }
}

// ---------------------------------------------------------------- tweaks

#[derive(Debug, PartialEq, Clone)]
enum Raw {
    Reg(Option<RegValue>, bool),
    Tcp(Vec<(String, Option<u32>)>),
    Svc(Option<(u32, bool, bool)>),
    Task(Option<bool>),
}

fn raw(a: &Action) -> Raw {
    match a {
        Action::Reg { hive, path, name, .. } | Action::RegFlag { hive, path, name, .. } => {
            Raw::Reg(registry::get(*hive, path, name), registry::key_exists(*hive, path))
        }
        Action::RegTcpInterfaces { name, .. } => Raw::Tcp(
            registry::subkeys(Hive::HKLM, TCP_INTERFACES)
                .into_iter()
                .map(|i| {
                    let v = registry::get_dword(Hive::HKLM, &format!(r"{TCP_INTERFACES}\{i}"), name);
                    (i, v)
                })
                .collect(),
        ),
        Action::Service { name, .. } => Raw::Svc(services::query(name).map(|s| (s.start_type, s.delayed, s.running))),
        Action::Task { path } => Raw::Task(tasks::is_enabled(path)),
    }
}

fn tweak_round_trips(r: &mut Report) {
    r.section("tweaks: apply -> verify -> revert -> byte-for-byte compare");
    // AVIX_SELFTEST_TWEAKS=id1,id2 limits the round trips to those tweaks
    let only: Vec<String> = std::env::var("AVIX_SELFTEST_TWEAKS")
        .map(|v| v.split(',').map(|x| x.trim().to_string()).filter(|x| !x.is_empty()).collect())
        .unwrap_or_default();
    for t in catalog::all() {
        if !only.is_empty() && !only.iter().any(|x| x == t.id) {
            continue;
        }
        let st = tweaks::state(t);
        if matches!(st, TweakState::Applied | TweakState::NotApplicable) {
            r.note(format!("skip {:<30} ({st:?} on this PC)", t.id));
            continue;
        }
        let before: Vec<Raw> = t.actions.iter().map(raw).collect();
        let a = tweaks::apply(t);
        let applied = tweaks::state(t);
        let v = tweaks::revert(t);
        // services may take a moment to start again
        let deadline = Instant::now() + Duration::from_secs(8);
        let mut after: Vec<Raw> = t.actions.iter().map(raw).collect();
        while after != before && Instant::now() < deadline {
            std::thread::sleep(Duration::from_millis(400));
            after = t.actions.iter().map(raw).collect();
        }
        let ok = a.is_ok() && applied == TweakState::Applied && v.is_ok() && after == before;
        let mut detail = format!("applied={applied:?}");
        if let Err(e) = &a {
            detail.push_str(&format!(" apply_err={e}"));
        }
        if let Err(e) = &v {
            detail.push_str(&format!(" revert_err={e}"));
        }
        r.check(&format!("{:<30}", t.id), ok, detail);
        for (i, (b, x)) in before.iter().zip(after.iter()).enumerate() {
            if b != x {
                r.note(format!("  action {i} differs: before {b:?} / after {x:?}"));
            }
        }
    }
}
