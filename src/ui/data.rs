//! Background data for the dashboard: system metrics, process table, GPU, ping,
//! and the engine connection. Runs only while the dashboard window is open.

use crate::ipc::{Client, Request, Response, Status};
use crate::sys::{gpu, memory, net, process};
use eframe::egui;
use std::collections::VecDeque;
use std::sync::mpsc::{channel, Receiver, RecvTimeoutError, Sender};
use std::sync::{Arc, Mutex, MutexGuard};
use std::time::{Duration, Instant};

pub const HISTORY: usize = 90;

#[derive(Clone, Debug)]
pub struct ProcRow {
    pub pid: u32,
    pub name: String,
    pub name_lc: String,
    pub cpu: f32,
    pub gpu: f32,
    pub mem: u64,
    pub priority: &'static str,
    pub user_session: bool,
}

#[derive(Default)]
pub struct Metrics {
    pub cpu: f32,
    pub cpu_hist: VecDeque<f32>,
    pub mem: memory::MemStatus,
    pub mem_hist: VecDeque<f32>,
    pub gpu: Option<f32>,
    pub gpu_hist: VecDeque<f32>,
    pub vram_used: u64,
    pub procs: Vec<ProcRow>,
    pub ping_target: String,
    pub ping_ms: Option<u32>,
    pub ping_hist: VecDeque<Option<u32>>,
    pub engine: Option<Status>,
}

#[derive(Clone, Copy, Debug, Default)]
pub struct PingStats {
    pub avg: f32,
    pub jitter: f32,
    pub loss: f32,
    pub samples: usize,
}

impl Metrics {
    pub fn ping_stats(&self) -> PingStats {
        let n = self.ping_hist.len();
        if n == 0 {
            return PingStats::default();
        }
        let ok: Vec<f32> = self.ping_hist.iter().flatten().map(|&v| v as f32).collect();
        let lost = n - ok.len();
        let avg = if ok.is_empty() { 0.0 } else { ok.iter().sum::<f32>() / ok.len() as f32 };
        let jitter = if ok.len() < 2 {
            0.0
        } else {
            ok.windows(2).map(|w| (w[1] - w[0]).abs()).sum::<f32>() / (ok.len() - 1) as f32
        };
        PingStats { avg, jitter, loss: lost as f32 / n as f32 * 100.0, samples: n }
    }
}

fn push<T>(q: &mut VecDeque<T>, v: T) {
    q.push_back(v);
    while q.len() > HISTORY {
        q.pop_front();
    }
}

pub fn lock<T>(m: &Mutex<T>) -> MutexGuard<'_, T> {
    m.lock().unwrap_or_else(|e| e.into_inner())
}

pub struct Monitor {
    pub metrics: Arc<Mutex<Metrics>>,
    tx: Sender<Request>,
    ping_host: Arc<Mutex<String>>,
}

impl Monitor {
    pub fn start(ctx: egui::Context, ping_host: String) -> Self {
        let metrics = Arc::new(Mutex::new(Metrics::default()));
        let (tx, rx) = channel::<Request>();
        let ping_host = Arc::new(Mutex::new(ping_host));

        let (m, c) = (metrics.clone(), ctx.clone());
        std::thread::Builder::new().name("avix-metrics".into()).spawn(move || metrics_loop(m, c, rx)).ok();
        let (m, c, h) = (metrics.clone(), ctx, ping_host.clone());
        std::thread::Builder::new().name("avix-ping".into()).spawn(move || ping_loop(m, c, h)).ok();

        Self { metrics, tx, ping_host }
    }

    pub fn send(&self, req: Request) {
        let _ = self.tx.send(req);
    }

    pub fn set_ping_host(&self, host: &str) {
        *lock(&self.ping_host) = host.to_string();
    }
}

fn metrics_loop(m: Arc<Mutex<Metrics>>, ctx: egui::Context, rx: Receiver<Request>) {
    let mut snap = process::Snapshotter::new();
    let mut cpu_t = process::CpuTracker::new();
    let mut sys_cpu = process::SystemCpu::default();
    let mut gpu_mon = gpu::GpuMonitor::new();
    let mut client: Option<Client> = None;
    let mut last_connect = Instant::now() - Duration::from_secs(10);
    let mut next_sample = Instant::now();
    sys_cpu.sample();
    // The first system CPU reading after start is unreliable (often 100 %); keep it out of history.
    let mut first_tick = true;

    loop {
        // Engine commands arrive here; otherwise this is our 1 s tick.
        let wait = next_sample.saturating_duration_since(Instant::now());
        let cmd = match rx.recv_timeout(wait) {
            Ok(c) => Some(c),
            Err(RecvTimeoutError::Timeout) => None,
            Err(RecvTimeoutError::Disconnected) => return,
        };

        if client.is_none() && (cmd.is_some() || last_connect.elapsed() > Duration::from_secs(2)) {
            last_connect = Instant::now();
            client = Client::connect();
        }
        if let Some(c) = cmd {
            if let Some(cl) = client.as_mut() {
                if cl.request(&c).is_none() {
                    client = None;
                }
            }
        }
        let engine = client.as_mut().and_then(|cl| match cl.request(&Request::Status) {
            Some(Response::Status { status }) => Some(*status),
            _ => None,
        });
        if engine.is_none() {
            client = None;
        }
        let engine = engine.map(|mut s| {
            if super::shot_admin() {
                // render as the normal, elevated app: the test engine's own "no admin" notice is
                // an artefact of how it was started for the screenshot
                s.elevated = true;
                s.events.retain(|e| !e.text.contains("without admin rights"));
            }
            #[cfg(debug_assertions)]
            if std::env::var_os("AVIX_SHOT_DEMO_BOOST").is_some() {
                // Debug screenshots only: a sample boost that changes things, so the boost card's
                // before → now rows can be checked without touching this PC's settings.
                let since = s.active.as_ref().map(|a| a.since_secs).unwrap_or(312);
                s.active = Some(crate::ipc::ActiveInfo {
                    profile: "Gaming".into(),
                    target: "avix-demo-game.exe".into(),
                    reason: "avix-demo-game.exe is running".into(),
                    since_secs: since,
                    power_plan: "Avix Ultimate Performance".into(),
                    prev_power_plan: "Balanced".into(),
                    throttled: 117,
                    raised: 2,
                    services_paused: vec!["wuauserv".into(), "UsoSvc".into(), "BITS".into()],
                    apps_closed: 1,
                    standby_purges: 1,
                    timer_resolution: true,
                    protected_target: true,
                    ..Default::default()
                });
            }
            s
        });

        let sample_now = Instant::now() >= next_sample;
        if sample_now {
            next_sample = Instant::now() + Duration::from_millis(1000);
            let procs = snap.take();
            let cpu_map = cpu_t.update(&procs);
            let g = gpu_mon.as_mut().map(|g| g.sample());
            let own_session = procs.iter().find(|p| p.pid == std::process::id()).map(|p| p.session).unwrap_or(1);
            let mut rows: Vec<ProcRow> = procs
                .iter()
                .filter(|p| p.pid > 0)
                .map(|p| ProcRow {
                    pid: p.pid,
                    name: p.name.clone(),
                    name_lc: p.name_lc.clone(),
                    cpu: cpu_map.get(&p.pid).copied().unwrap_or(0.0),
                    gpu: g.as_ref().and_then(|g| g.per_pid.get(&p.pid).copied()).unwrap_or(0.0),
                    mem: p.private_ws,
                    priority: p.priority_label(),
                    user_session: p.session == own_session,
                })
                .collect();
            rows.sort_by(|a, b| b.cpu.partial_cmp(&a.cpu).unwrap_or(std::cmp::Ordering::Equal).then(b.mem.cmp(&a.mem)));
            let cpu = sys_cpu.sample();
            let mem = memory::status();

            let mut mm = lock(&m);
            mm.cpu = cpu;
            if !first_tick {
                push(&mut mm.cpu_hist, cpu);
            }
            first_tick = false;
            mm.mem = mem;
            push(&mut mm.mem_hist, mem.load as f32);
            if let Some(g) = g {
                mm.gpu = Some(g.utilization);
                push(&mut mm.gpu_hist, g.utilization);
                mm.vram_used = g.dedicated_used;
            }
            mm.procs = rows;
            mm.engine = engine;
        } else {
            lock(&m).engine = engine;
        }
        ctx.request_repaint();
    }
}

fn ping_loop(m: Arc<Mutex<Metrics>>, ctx: egui::Context, host: Arc<Mutex<String>>) {
    let mut pinger: Option<(String, net::Pinger)> = None;
    loop {
        let want = lock(&host).clone();
        if pinger.as_ref().map(|(h, _)| h != &want).unwrap_or(true) {
            pinger = net::Pinger::new(&want).map(|p| (want.clone(), p));
            let mut mm = lock(&m);
            mm.ping_hist.clear();
            mm.ping_target = want.clone();
        }
        let started = Instant::now();
        match &pinger {
            Some((_, p)) => {
                let r = p.ping(1000);
                let mut mm = lock(&m);
                mm.ping_ms = r;
                push(&mut mm.ping_hist, r);
            }
            None => {
                lock(&m).ping_ms = None;
                std::thread::sleep(Duration::from_secs(4));
            }
        }
        ctx.request_repaint();
        std::thread::sleep(Duration::from_millis(1000).saturating_sub(started.elapsed()));
    }
}

// ---------------------------------------------------------------- background jobs

pub struct Job<T> {
    rx: Receiver<T>,
}

impl<T: Send + 'static> Job<T> {
    pub fn spawn(ctx: &egui::Context, f: impl FnOnce() -> T + Send + 'static) -> Self {
        let (tx, rx) = channel();
        let ctx = ctx.clone();
        std::thread::spawn(move || {
            let r = f();
            let _ = tx.send(r);
            ctx.request_repaint();
        });
        Self { rx }
    }
}

/// Take a finished job's result (clearing the slot). Panicked jobs just clear.
pub fn take<T>(slot: &mut Option<Job<T>>) -> Option<T> {
    let job = slot.as_ref()?;
    match job.rx.try_recv() {
        Ok(v) => {
            *slot = None;
            Some(v)
        }
        Err(std::sync::mpsc::TryRecvError::Disconnected) => {
            *slot = None;
            None
        }
        Err(std::sync::mpsc::TryRecvError::Empty) => None,
    }
}
