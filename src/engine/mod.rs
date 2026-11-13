//! The background engine: decides when to boost, applies the boost, keeps it
//! applied to newly started apps, and restores everything exactly afterwards.

pub mod journal;
mod tray;

use crate::config::{self, Config, PowerPlan, Profile, TargetPriority};
use crate::icon::IconState;
use crate::ipc::{self, ActiveInfo, Event, EventKind, Mode, Request, Response, Status};
use crate::logging::log;
use crate::sys::process::{self, ProcHandle, ProcInfo, Snapshotter, SystemCpu};
use crate::sys::games::{self, KnownGame, LibraryRoot};
use crate::sys::gpu::{GpuMonitor, GpuSample};
use crate::sys::{self, memory, ntdll, power, services, window};
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet, VecDeque};
use std::sync::atomic::AtomicUsize;
use std::sync::{Arc, Mutex, MutexGuard};
use std::time::{Duration, Instant};
use windows_sys::core::GUID;

/// Title of the dashboard window (used to bring it to the front).
pub const UI_TITLE: &str = "Avix Optimizer";

const BUSY_GRACE: Duration = Duration::from_secs(45);
const PURGE_COOLDOWN: Duration = Duration::from_secs(60);
/// Background apps' memory is freed at most this often (only while RAM is short).
const TRIM_COOLDOWN: Duration = Duration::from_secs(60);
/// How often Windows' game list and the store libraries are re-read (new installs, first launches).
const KNOWN_GAMES_REFRESH: Duration = Duration::from_secs(30);
/// 3D-engine use (% of the GPU) that marks the app you're using as a game...
const GPU_BUSY_PERCENT: f32 = 20.0;
/// ...and below which a GPU-detected game counts as idle.
const GPU_IDLE_PERCENT: f32 = 5.0;
/// A GPU-detected game that stays idle this long stops being boosted.
const GPU_IDLE_END: Duration = Duration::from_secs(120);
/// An app that wasn't busy enough is re-checked after this long.
const GPU_RETRY: Duration = Duration::from_secs(120);
/// The GPU counters are closed again when unused this long (keeps the engine tiny).
const GPU_RELEASE: Duration = Duration::from_secs(60);

/// How a game that isn't in any profile was recognised.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum GameSource {
    WindowsList,
    Library(&'static str),
    Gpu,
}

impl GameSource {
    fn reason(self, name: &str) -> String {
        match self {
            GameSource::WindowsList => format!("{name} is running - Windows recognises it as a game"),
            GameSource::Library(s) if s.starts_with("a ") => format!("{name} is running from {s}"),
            GameSource::Library(s) => format!("{name} is running - installed from {s}"),
            GameSource::Gpu => format!("{name} is keeping the GPU busy like a game"),
        }
    }
}

#[derive(Clone, Copy)]
struct GpuProbe {
    pid: u32,
    create: i64,
    samples: u8,
    busy: u8,
}

// ------------------------------------------------------------ single instance

pub struct SingleInstance(#[allow(dead_code)] sys::OwnedHandle);

impl SingleInstance {
    pub fn acquire() -> Option<Self> {
        use windows_sys::Win32::Foundation::ERROR_ALREADY_EXISTS;
        use windows_sys::Win32::System::Threading::CreateMutexW;
        let name = sys::wide(&format!("Local\\Avix.Engine.v1{}", ipc::instance_suffix()));
        let h = unsafe { CreateMutexW(std::ptr::null(), 0, name.as_ptr()) };
        let err = sys::last_error();
        let h = sys::OwnedHandle::new(h)?;
        (err != ERROR_ALREADY_EXISTS).then_some(Self(h))
    }
}

// ------------------------------------------------------------ shared with IPC

pub enum Command {
    SetMode(Mode, Option<String>),
    ReloadConfig,
    PurgeStandby,
    OpenUi,
    Shutdown,
    CheckUpdate,
}

#[derive(Default)]
pub struct Shared {
    pub status: Mutex<Status>,
    pub commands: Mutex<Vec<Command>>,
    pub hwnd: AtomicUsize,
}

fn lock<T>(m: &Mutex<T>) -> MutexGuard<'_, T> {
    m.lock().unwrap_or_else(|e| e.into_inner())
}

pub fn run(guard: SingleInstance, open_ui: bool, paused: bool) {
    journal::restore_from_disk();
    let shared = Arc::new(Shared::default());
    let mut engine = Engine::new(shared.clone());
    if paused {
        engine.mode = Mode::Off;
    }
    let s2 = shared.clone();
    ipc::start_server(move |req| handle_request(&s2, req));
    tray::run(engine, open_ui);
    drop(guard);
    log("Engine stopped");
}

fn handle_request(shared: &Shared, req: Request) -> Response {
    let cmd = match req {
        Request::Status => {
            return Response::Status { status: Box::new(lock(&shared.status).clone()) };
        }
        Request::SetMode { mode, profile } => Command::SetMode(mode, profile),
        Request::ReloadConfig => Command::ReloadConfig,
        Request::PurgeStandby => Command::PurgeStandby,
        Request::OpenUi => Command::OpenUi,
        Request::Shutdown => Command::Shutdown,
        Request::CheckUpdate => Command::CheckUpdate,
    };
    lock(&shared.commands).push(cmd);
    tray::wake(shared);
    Response::Ok
}

pub fn spawn_ui() -> Option<std::process::Child> {
    let exe = std::env::current_exe().ok()?;
    std::process::Command::new(exe).args(["--ui", "--no-elevate"]).spawn().ok()
}

// ------------------------------------------------------------ touched processes

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Role {
    Background,
    Target,
    Companion,
}

/// Original settings of a process Avix modified, so it can be put back exactly.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Touched {
    pub pid: u32,
    pub create_time: i64,
    pub name: String,
    pub role: Role,
    pub priority: Option<u32>,
    pub throttle: Option<(u32, u32)>,
    pub mem_priority: Option<u32>,
    pub io_priority: Option<u32>,
}

impl Touched {
    fn new(pid: u32, create_time: i64, name: &str, role: Role) -> Self {
        Self {
            pid,
            create_time,
            name: name.to_string(),
            role,
            priority: None,
            throttle: None,
            mem_priority: None,
            io_priority: None,
        }
    }

    fn is_empty(&self) -> bool {
        self.priority.is_none() && self.throttle.is_none() && self.mem_priority.is_none() && self.io_priority.is_none()
    }

    /// Restore original settings. Skips silently if the PID now belongs to another process.
    pub fn restore(&self) -> bool {
        let Some(h) = ProcHandle::open_rw(self.pid) else { return false };
        if h.create_time() != Some(self.create_time) {
            return false;
        }
        if let Some(p) = self.priority {
            h.set_priority_class(p);
        }
        if let Some((c, s)) = self.throttle {
            h.set_power_throttling(c, s);
        }
        if let Some(m) = self.mem_priority {
            h.set_memory_priority(m);
        }
        if let Some(io) = self.io_priority {
            h.set_io_priority(io);
        }
        true
    }
}

fn throttle_process(p: &ProcInfo, profile: &Profile) -> Option<Touched> {
    let h = ProcHandle::open_rw(p.pid)?;
    let mut t = Touched::new(p.pid, h.create_time()?, &p.name, Role::Background);
    if let Some(c) = h.priority_class() {
        if (c == process::PRIO_NORMAL || c == process::PRIO_ABOVE_NORMAL) && h.set_priority_class(process::PRIO_BELOW_NORMAL) {
            t.priority = Some(c);
        }
    }
    if profile.efficiency_mode {
        let orig = h.power_throttling().unwrap_or((0, 0));
        if h.set_efficiency_mode(true) {
            t.throttle = Some(orig);
        }
    }
    if let Some(m) = h.memory_priority() {
        if m > process::MEMORY_PRIORITY_BELOW_NORMAL && h.set_memory_priority(process::MEMORY_PRIORITY_BELOW_NORMAL) {
            t.mem_priority = Some(m);
        }
    }
    if profile.lower_io_priority {
        if let Some(io) = h.io_priority() {
            if io >= process::IO_PRIORITY_NORMAL && h.set_io_priority(process::IO_PRIORITY_LOW) {
                t.io_priority = Some(io);
            }
        }
    }
    (!t.is_empty()).then_some(t)
}

/// Make a process snappy: given priority class + never EcoQoS (and honour its timer requests).
fn raise_process(pid: u32, name: &str, class: Option<u32>, role: Role) -> Option<Touched> {
    let h = ProcHandle::open_rw(pid)?;
    let mut t = Touched::new(pid, h.create_time()?, name, role);
    if let Some(class) = class {
        if let Some(orig) = h.priority_class() {
            if orig != class && orig != process::PRIO_HIGH && h.set_priority_class(class) {
                t.priority = Some(orig);
            }
        }
    }
    let orig = h.power_throttling().unwrap_or((0, 0));
    if h.set_power_throttling(process::THROTTLE_EXECUTION_SPEED | process::THROTTLE_IGNORE_TIMER_RESOLUTION, 0) {
        t.throttle = Some(orig);
    }
    Some(t)
}

// ------------------------------------------------------------ engine

#[derive(Clone)]
struct Desired {
    profile: Profile,
    pid: u32,
    name: String,
    create: i64,
    reason: String,
    from_fullscreen: bool,
    manual: bool,
}

struct Active {
    profile: Profile,
    manual: bool,
    from_fullscreen: bool,
    target_pid: u32,
    target_name: String,
    target_create: i64,
    target_touched: Option<Touched>,
    protected_target: bool,
    reason: String,
    since: Instant,
    prev_plan: Option<GUID>,
    plan_applied: Option<GUID>,
    plan_name: String,
    touched: HashMap<u32, Touched>,
    skip: HashSet<u32>,
    fg_exempt: Option<u32>,
    stopped_services: Vec<String>,
    timer_set: bool,
    purges: u32,
    closed: u32,
    journal_dirty: bool,
}

pub struct Engine {
    pub shared: Arc<Shared>,
    cfg: Config,
    mode: Mode,
    forced_profile: Option<String>,
    snap: Snapshotter,
    procs: Vec<ProcInfo>,
    own_pid: u32,
    own_session: u32,
    elevated: bool,
    active: Option<Active>,
    events: VecDeque<Event>,
    fg_pid: u32,
    fg_name: String,
    fs_candidate: Option<(u32, u8)>,
    trigger_cpu: HashMap<u32, (u64, Instant)>,
    last_busy: HashMap<u32, Instant>,
    windows: Vec<window::AppWindow>,
    known_games: Vec<KnownGame>,
    library_roots: Vec<LibraryRoot>,
    lists_at: Option<Instant>,
    /// pid -> (create time, lowercase exe path if readable); each process is looked up once
    paths: HashMap<u32, (i64, Option<String>)>,
    /// Running games that aren't in a profile: pid -> (create time, how we know)
    games: HashMap<u32, (i64, GameSource)>,
    gpu: Option<GpuMonitor>,
    gpu_sampled: Option<Instant>,
    gpu_probe: Option<GpuProbe>,
    gpu_rejected: HashMap<u32, (i64, Instant)>,
    gpu_busy: HashMap<u32, Instant>,
    sys_cpu: SystemCpu,
    cpu_percent: f32,
    own_cpu: Option<(u64, Instant)>,
    own_cpu_percent: f32,
    lists: Option<(u64, u64)>,
    started: Instant,
    ui_child: Option<std::process::Child>,
    detected: Vec<String>,
    warned_protected: HashSet<String>,
    last_purge: Option<Instant>,
    last_tick: Option<Instant>,
    balloon: Option<(String, String)>,
    updater: crate::update::Checker,
    /// integrated + dedicated GPU (laptops): games get the dedicated one
    hybrid_gpu: bool,
    last_trim: Option<Instant>,
    update_force: bool,
    /// a found update waiting to be announced (not mid-game)
    update_pending: Option<ipc::UpdateInfo>,
    tick_n: u64,
    shut_down: bool,
}

impl Engine {
    pub fn new(shared: Arc<Shared>) -> Self {
        let elevated = sys::elevation::is_elevated();
        if elevated {
            memory::enable_privilege("SeDebugPrivilege");
            memory::enable_privilege("SeProfileSingleProcessPrivilege");
        }
        let cfg = Config::load();
        let own_pid = std::process::id();
        let mut snap = Snapshotter::new();
        let procs = snap.take();
        let own_session = procs.iter().find(|p| p.pid == own_pid).map(|p| p.session).unwrap_or(1);
        let mut e = Self {
            shared,
            cfg,
            mode: Mode::Auto,
            forced_profile: None,
            snap,
            procs,
            own_pid,
            own_session,
            elevated,
            active: None,
            events: VecDeque::new(),
            fg_pid: 0,
            fg_name: String::new(),
            fs_candidate: None,
            trigger_cpu: HashMap::new(),
            last_busy: HashMap::new(),
            windows: Vec::new(),
            known_games: Vec::new(),
            library_roots: Vec::new(),
            lists_at: None,
            paths: HashMap::new(),
            games: HashMap::new(),
            gpu: None,
            gpu_sampled: None,
            gpu_probe: None,
            gpu_rejected: HashMap::new(),
            gpu_busy: HashMap::new(),
            sys_cpu: SystemCpu::default(),
            cpu_percent: 0.0,
            own_cpu: None,
            own_cpu_percent: 0.0,
            lists: None,
            started: Instant::now(),
            ui_child: None,
            detected: Vec::new(),
            warned_protected: HashSet::new(),
            last_purge: None,
            last_tick: None,
            balloon: None,
            tick_n: 0,
            shut_down: false,
            updater: crate::update::Checker::new(),
            hybrid_gpu: sys::hardware::is_hybrid(&sys::hardware::gpus()) || cfg!(debug_assertions) && std::env::var_os("AVIX_FAKE_HYBRID").is_some(),
            last_trim: None,
            update_force: false,
            update_pending: None,
        };
        e.sys_cpu.sample();
        if elevated {
            e.event(EventKind::Info, "Engine started - watching for games & heavy apps".into());
        } else {
            e.event(
                EventKind::Warn,
                "Engine started WITHOUT admin rights - services, standby cleaner and system processes are off".into(),
            );
        }
        e
    }

    pub fn poll_ms(&self) -> u64 {
        self.cfg.poll_interval_ms
    }

    pub fn hotkey_enabled(&self) -> bool {
        self.cfg.hotkey
    }

    fn event(&mut self, kind: EventKind, text: String) {
        log(&text);
        self.events.push_back(Event { time: sys::local_time(), kind, text });
        while self.events.len() > 80 {
            self.events.pop_front();
        }
    }

    fn proc_by_pid(&self, pid: u32) -> Option<&ProcInfo> {
        self.procs.iter().find(|p| p.pid == pid)
    }

    // -------------------------------------------------------- main loop

    pub fn tick(&mut self) {
        if self.shut_down {
            return;
        }
        self.tick_n += 1;
        self.last_tick = Some(Instant::now());
        self.procs = self.snap.take();
        self.windows = window::app_windows();
        self.update_foreground();
        self.update_trigger_cpu();
        self.update_games();
        self.cpu_percent = self.sys_cpu.sample();

        let desired = self.compute_desired();
        let action = match (&self.active, &desired) {
            (None, None) => 0,
            (None, Some(_)) => 1,
            (Some(_), None) => 2,
            (Some(a), Some(d)) => {
                if a.profile.name != d.profile.name || a.manual != d.manual {
                    3
                } else if a.target_pid != d.pid {
                    4
                } else {
                    0
                }
            }
        };
        match (action, desired) {
            (1, Some(d)) => self.activate(d),
            (2, _) => self.deactivate(),
            (3, Some(d)) => {
                self.deactivate();
                self.activate(d);
            }
            (4, Some(d)) => self.retarget(d),
            _ => {}
        }
        if self.active.is_some() {
            self.maintain();
        }
        self.update_tick();
        self.publish();
    }

    /// Daily update check; a new version is announced once, and never in the middle of a game.
    fn update_tick(&mut self) {
        let force = std::mem::take(&mut self.update_force);
        // only after the user said yes on the first-run screen
        if let Some(info) = self.updater.tick(self.cfg.check_updates && self.cfg.welcomed, force) {
            self.event(
                EventKind::Info,
                format!("Avix {} is available (you have {}) - get it from the tray menu or the dashboard", info.version, crate::update::current_version()),
            );
            self.update_pending = Some(info);
        }
        if self.active.is_none() {
            if let Some(info) = self.update_pending.take() {
                // test instances (debug AVIX_INSTANCE) stay quiet
                if ipc::instance_suffix().is_empty() {
                    self.balloon = Some((format!("Avix {} is available", info.version), "Click here to open the download page".into()));
                }
            }
        }
    }

    pub fn update_available(&self) -> Option<ipc::UpdateInfo> {
        self.updater.available()
    }

    /// Foreground changed (WinEvent hook) - react quickly, but not more than 4x/second.
    pub fn on_foreground(&mut self) {
        if self.last_tick.is_some_and(|t| t.elapsed() < Duration::from_millis(250)) {
            return;
        }
        self.tick();
    }

    fn update_foreground(&mut self) {
        let fg = window::foreground();
        self.fg_pid = fg.map(|(_, pid)| pid).unwrap_or(0);
        self.fg_name = self.proc_by_pid(self.fg_pid).map(|p| p.name.clone()).unwrap_or_default();

        // Fullscreen candidate: the focused window first, otherwise any fullscreen app window
        // (e.g. a game behind the dashboard, or on another monitor). Needs two sightings in a row.
        let ok = |pid: u32| {
            self.proc_by_pid(pid).is_some_and(|p| {
                pid != self.own_pid
                    && p.session == self.own_session
                    && !games::is_not_game(&p.name_lc)
                    && !config::contains(&self.cfg.fullscreen_ignore, &p.name_lc)
                    && !config::contains(&self.cfg.never_touch, &p.name_lc)
            })
        };
        let cand = fg
            .filter(|(h, _)| window::is_fullscreen(*h))
            .map(|(_, pid)| pid)
            .filter(|&pid| ok(pid))
            .or_else(|| self.windows.iter().filter(|w| w.fullscreen).map(|w| w.pid).find(|&pid| ok(pid)));
        self.fs_candidate = match (cand, self.fs_candidate) {
            (Some(p), Some((q, n))) if p == q => Some((p, n.saturating_add(1))),
            (Some(p), _) => Some((p, 1)),
            (None, _) => None,
        };
    }

    /// Track CPU use of trigger processes for profiles that only boost while busy.
    fn update_trigger_cpu(&mut self) {
        let now = Instant::now();
        let watched: Vec<(&Vec<String>, f32)> = self
            .cfg
            .profiles
            .iter()
            .filter(|p| p.enabled && p.min_cpu_core_percent > 0.0)
            .map(|p| (&p.triggers, p.min_cpu_core_percent))
            .collect();
        if watched.is_empty() {
            self.trigger_cpu.clear();
            return;
        }
        let mut seen = HashSet::new();
        for p in &self.procs {
            let Some(threshold) = watched
                .iter()
                .filter(|(t, _)| config::contains(t, &p.name_lc))
                .map(|(_, th)| *th)
                .reduce(f32::min)
            else {
                continue;
            };
            seen.insert(p.pid);
            if let Some((cpu, at)) = self.trigger_cpu.insert(p.pid, (p.cpu_time, now)) {
                let secs = now.duration_since(at).as_secs_f64();
                if secs > 0.1 {
                    let core_pct = p.cpu_time.saturating_sub(cpu) as f64 / 1e7 / secs * 100.0;
                    if core_pct >= threshold as f64 {
                        self.last_busy.insert(p.pid, now);
                    }
                }
            }
        }
        self.trigger_cpu.retain(|pid, _| seen.contains(pid));
        self.last_busy.retain(|pid, t| seen.contains(pid) && now.duration_since(*t) < BUSY_GRACE);
    }

    /// Could this process be a game nobody put in a profile? (Cheap checks only.)
    fn detectable(&self, p: &ProcInfo) -> bool {
        p.pid > 4
            && p.pid != self.own_pid
            && p.session == self.own_session
            && !games::is_not_game(&p.name_lc)
            && !config::contains(&self.cfg.never_touch, &p.name_lc)
            && !config::contains(&self.cfg.fullscreen_ignore, &p.name_lc)
            // apps named in a profile follow that profile's rules (e.g. AI apps boost only while busy)
            && !self.cfg.profiles.iter().any(|pr| config::contains(&pr.triggers, &p.name_lc))
    }

    /// Find running games that aren't in any profile - whether they started before or after
    /// Avix, fullscreen or windowed - from Windows' own game list, the game stores' install
    /// folders, and (for the app you're using) how hard it drives the GPU's 3D engine.
    fn update_games(&mut self) {
        let now = Instant::now();
        let (use_list, use_libs, use_gpu) = (self.cfg.windows_games, self.cfg.library_games, self.cfg.gpu_games);
        if !use_list {
            self.known_games.clear();
        }
        if !use_libs {
            self.library_roots.clear();
        }
        if (use_list || use_libs) && self.lists_at.map_or(true, |t| now.duration_since(t) >= KNOWN_GAMES_REFRESH) {
            if use_list {
                self.known_games = games::windows_known();
            }
            if use_libs {
                self.library_roots = games::library_roots();
            }
            self.lists_at = Some(now);
        }

        // forget processes that exited (or whose pid was reused)
        let alive: HashMap<u32, i64> = self.procs.iter().map(|p| (p.pid, p.create_time)).collect();
        self.paths.retain(|pid, (ct, _)| alive.get(pid) == Some(ct));
        self.gpu_rejected.retain(|pid, (ct, _)| alive.get(pid) == Some(ct));
        self.gpu_busy.retain(|pid, _| alive.contains_key(pid));
        self.games.retain(|pid, (ct, src)| {
            alive.get(pid) == Some(ct)
                && match src {
                    GameSource::WindowsList => use_list,
                    GameSource::Library(_) => use_libs,
                    GameSource::Gpu => use_gpu,
                }
        });

        // a store-installed app counts once it shows a real window (skips helpers and installers)
        let windowed: HashSet<u32> =
            self.windows.iter().filter(|w| w.width >= 400 && w.height >= 300).map(|w| w.pid).collect();
        let mut found = Vec::new();
        for p in &self.procs {
            if self.games.contains_key(&p.pid) {
                continue;
            }
            let known: Vec<&KnownGame> = self.known_games.iter().filter(|g| g.name_lc == p.name_lc).collect();
            let has_window = use_libs && windowed.contains(&p.pid);
            if (known.is_empty() && !has_window) || !self.detectable(p) {
                continue;
            }
            // kernel anti-cheat: never open a handle to the game, its name has to do
            let protected = config::contains(&self.cfg.anticheat_protected, &p.name_lc);
            let path = match self.paths.get(&p.pid) {
                Some((_, path)) => path.clone(),
                None => {
                    let path = if protected { None } else { process::image_path(p.pid).map(|s| s.to_lowercase()) };
                    self.paths.insert(p.pid, (p.create_time, path.clone()));
                    path
                }
            };
            let listed = !known.is_empty()
                && match &path {
                    Some(lc) => known.iter().any(|g| &g.path_lc == lc),
                    None => protected || known.iter().any(|g| g.name_is_distinctive()),
                };
            let src = if listed {
                Some(GameSource::WindowsList)
            } else if has_window {
                path.as_deref().and_then(|lc| games::library_of(lc, &self.library_roots)).map(GameSource::Library)
            } else {
                None
            };
            if let Some(src) = src {
                found.push((p.pid, p.create_time, src));
            }
        }
        for (pid, ct, src) in found {
            self.games.insert(pid, (ct, src));
        }
        self.update_gpu_games(now);
    }

    /// The app you're using, if it's a candidate for the GPU check: big window, not known yet.
    fn gpu_candidate(&self) -> Option<(u32, i64)> {
        let p = self.proc_by_pid(self.fg_pid)?;
        if self.games.contains_key(&p.pid) || !self.detectable(p) {
            return None;
        }
        if self.gpu_rejected.get(&p.pid).is_some_and(|(ct, at)| *ct == p.create_time && at.elapsed() < GPU_RETRY) {
            return None;
        }
        let big = self.windows.iter().any(|w| w.pid == p.pid && (w.fullscreen || (w.width >= 800 && w.height >= 500)));
        big.then_some((p.pid, p.create_time))
    }

    /// One GPU sample, at most every 1.5 s. The counters are opened on first use (that first
    /// collection only primes them) and closed again once unused for a minute.
    fn gpu_sample(&mut self, now: Instant) -> Option<GpuSample> {
        if self.gpu_sampled.is_some_and(|t| now.duration_since(t) < Duration::from_millis(1500)) {
            return None;
        }
        self.gpu_sampled = Some(now);
        match self.gpu.as_mut() {
            Some(g) => Some(g.sample()),
            None => {
                let mut g = GpuMonitor::new()?;
                g.sample();
                self.gpu = Some(g);
                None
            }
        }
    }

    /// Games from anywhere (no store, no list, windowed): the app you're using counts as a game
    /// once its 3D-engine use stays high, and stops counting after it idles for a while.
    fn update_gpu_games(&mut self, now: Instant) {
        let auto = self.cfg.gpu_games && self.mode == Mode::Auto;
        let target = self
            .active
            .as_ref()
            .filter(|a| auto && !a.manual)
            .map(|a| a.target_pid)
            .filter(|pid| matches!(self.games.get(pid), Some((_, GameSource::Gpu))));
        let probe = if auto && self.active.is_none() { self.gpu_candidate() } else { None };
        if probe.is_none() {
            self.gpu_probe = None;
        }
        // a boosted GPU game only needs a check every ~10 s
        let due = probe.is_some()
            || (target.is_some() && self.gpu_sampled.map_or(true, |t| now.duration_since(t) >= Duration::from_secs(10)));
        if !due {
            if self.gpu.is_some() && self.gpu_sampled.map_or(true, |t| now.duration_since(t) >= GPU_RELEASE) {
                self.gpu = None;
            }
            return;
        }
        let Some(sample) = self.gpu_sample(now) else { return };
        let use_of = |pid: u32| sample.per_pid_3d.get(&pid).copied().unwrap_or(0.0);

        if let Some(pid) = target {
            if use_of(pid) >= GPU_IDLE_PERCENT {
                self.gpu_busy.insert(pid, now);
            }
            if self.gpu_busy.get(&pid).map_or(true, |t| now.duration_since(*t) >= GPU_IDLE_END) {
                self.games.remove(&pid);
                let name = self.proc_by_pid(pid).map(|p| p.name.clone()).unwrap_or_default();
                self.event(EventKind::Info, format!("{name} stopped using the GPU - no longer boosting it"));
            }
        }
        if let Some((pid, create)) = probe {
            let mut pr = match self.gpu_probe {
                Some(pr) if pr.pid == pid && pr.create == create => pr,
                _ => GpuProbe { pid, create, samples: 0, busy: 0 },
            };
            pr.samples += 1;
            if use_of(pid) >= GPU_BUSY_PERCENT {
                pr.busy += 1;
            }
            self.gpu_probe = None;
            if pr.busy >= 3 {
                self.games.insert(pid, (create, GameSource::Gpu));
                self.gpu_busy.insert(pid, now);
                let name = self.proc_by_pid(pid).map(|p| p.name.clone()).unwrap_or_default();
                if !self.detected.iter().any(|x| x.eq_ignore_ascii_case(&name)) {
                    self.detected.push(name.clone());
                    self.event(
                        EventKind::Info,
                        format!("{name} keeps the GPU busy, so Avix treats it as a game - add it to a profile so it boosts instantly"),
                    );
                }
            } else if pr.samples >= 6 {
                self.gpu_rejected.insert(pid, (create, now));
            } else {
                self.gpu_probe = Some(pr);
            }
        }
    }

    fn eligible_fg_target(&self) -> Option<&ProcInfo> {
        let p = self.proc_by_pid(self.fg_pid)?;
        let bad = p.pid == self.own_pid
            || p.pid <= 4
            || config::contains(&self.cfg.never_touch, &p.name_lc)
            || config::contains(&self.cfg.fullscreen_ignore, &p.name_lc);
        (!bad).then_some(p)
    }

    fn compute_desired(&mut self) -> Option<Desired> {
        match self.mode {
            Mode::Off => None,
            Mode::On => {
                let profile = self
                    .forced_profile
                    .as_ref()
                    .and_then(|n| self.cfg.profile(n))
                    .or_else(|| self.cfg.profiles.iter().find(|p| p.enabled))
                    .cloned()
                    .unwrap_or_default();
                // target follows the foreground app; otherwise keep the previous target while alive
                let target = self.eligible_fg_target().map(|p| (p.pid, p.name.clone(), p.create_time)).or_else(|| {
                    let a = self.active.as_ref().filter(|a| a.manual && a.target_pid != 0)?;
                    let p = self.procs.iter().find(|p| p.pid == a.target_pid && p.create_time == a.target_create)?;
                    Some((p.pid, p.name.clone(), p.create_time))
                });
                let (pid, name, create) = target.unwrap_or((0, String::new(), 0));
                Some(Desired {
                    profile,
                    pid,
                    name,
                    create,
                    reason: "Manual boost".into(),
                    from_fullscreen: false,
                    manual: true,
                })
            }
            Mode::Auto => self.auto_desired(),
        }
    }

    fn auto_desired(&mut self) -> Option<Desired> {
        let now = Instant::now();
        let mut best: Option<(usize, Desired)> = None;

        for (i, prof) in self.cfg.profiles.iter().enumerate() {
            // the profile for detected games also picks up games Windows has recognised
            let games_profile = !self.games.is_empty() && prof.name.eq_ignore_ascii_case(&self.cfg.fullscreen_profile);
            if !prof.enabled || (prof.triggers.is_empty() && !games_profile) {
                continue;
            }
            let mut cands: Vec<&ProcInfo> = self
                .procs
                .iter()
                .filter(|x| {
                    config::contains(&prof.triggers, &x.name_lc) || (games_profile && self.games.contains_key(&x.pid))
                })
                .collect();
            if prof.min_cpu_core_percent > 0.0 {
                cands.retain(|x| self.last_busy.get(&x.pid).is_some_and(|t| now.duration_since(*t) < BUSY_GRACE));
            }
            if cands.is_empty() {
                continue;
            }
            let current = self.active.as_ref().map(|a| a.target_pid).unwrap_or(0);
            let pick = cands
                .iter()
                .find(|x| x.pid == current)
                .or_else(|| cands.iter().find(|x| x.pid == self.fg_pid))
                .or_else(|| cands.iter().max_by_key(|x| x.working_set))
                .copied()
                .expect("non-empty");
            let reason = match self.games.get(&pick.pid) {
                Some((_, src)) if !config::contains(&prof.triggers, &pick.name_lc) => src.reason(&pick.name),
                _ => format!("{} is running", pick.name),
            };
            best = Some((
                i,
                Desired {
                    profile: prof.clone(),
                    pid: pick.pid,
                    name: pick.name.clone(),
                    create: pick.create_time,
                    reason,
                    from_fullscreen: false,
                    manual: false,
                },
            ));
            break;
        }

        if self.cfg.auto_fullscreen {
            let fs_idx = self
                .cfg
                .profiles
                .iter()
                .position(|p| p.enabled && p.name.eq_ignore_ascii_case(&self.cfg.fullscreen_profile));
            if let Some(fi) = fs_idx {
                // sticky: keep a fullscreen-detected game boosted while it runs (even alt-tabbed)
                let sticky = self.active.as_ref().filter(|a| a.from_fullscreen && !a.manual).and_then(|a| {
                    self.procs.iter().find(|x| x.pid == a.target_pid && x.create_time == a.target_create)
                });
                let fresh = self
                    .fs_candidate
                    .filter(|(_, n)| *n >= 2)
                    .and_then(|(pid, _)| self.proc_by_pid(pid));
                if let Some(c) = sticky.or(fresh) {
                    if best.as_ref().map_or(true, |(bi, _)| fi < *bi) {
                        let in_profile = self.cfg.profiles.iter().any(|p| config::contains(&p.triggers, &c.name_lc));
                        let name = c.name.clone();
                        let d = Desired {
                            profile: self.cfg.profiles[fi].clone(),
                            pid: c.pid,
                            name: name.clone(),
                            create: c.create_time,
                            reason: format!("{name} is fullscreen"),
                            from_fullscreen: true,
                            manual: false,
                        };
                        if !in_profile && !self.detected.iter().any(|x| x.eq_ignore_ascii_case(&name)) {
                            self.detected.push(name.clone());
                            self.event(
                                EventKind::Info,
                                format!("Detected fullscreen app {name} - add it to a profile so it boosts instantly"),
                            );
                        }
                        best = Some((fi, d));
                    }
                }
            }
        }
        best.map(|(_, d)| d)
    }

    // -------------------------------------------------------- boost on / off

    fn activate(&mut self, d: Desired) {
        let mut a = Active {
            profile: d.profile.clone(),
            manual: d.manual,
            from_fullscreen: d.from_fullscreen,
            target_pid: d.pid,
            target_name: d.name.clone(),
            target_create: d.create,
            target_touched: None,
            protected_target: false,
            reason: d.reason.clone(),
            since: Instant::now(),
            prev_plan: None,
            plan_applied: None,
            plan_name: String::new(),
            touched: HashMap::new(),
            skip: HashSet::new(),
            fg_exempt: None,
            stopped_services: Vec::new(),
            timer_set: false,
            purges: 0,
            closed: 0,
            journal_dirty: true,
        };
        let mut warnings = Vec::new();

        // 1. power plan
        let plan = match d.profile.power_plan {
            PowerPlan::Keep => None,
            PowerPlan::HighPerformance => Some(Ok(power::HIGH_PERFORMANCE)),
            PowerPlan::AvixUltimate => Some(power::ensure_avix_plan()),
        };
        match plan {
            Some(Ok(g)) => {
                let prev = power::active();
                if prev.map_or(true, |p| !power::same(&p, &g)) {
                    match power::set_active(&g) {
                        Ok(()) => {
                            a.prev_plan = prev;
                            a.plan_applied = Some(g);
                        }
                        Err(e) => warnings.push(format!("Power plan switch failed: {e}")),
                    }
                }
                a.plan_name = power::name_of(&g);
            }
            Some(Err(e)) => warnings.push(format!("Power plan unavailable: {e}")),
            None => a.plan_name = power::active().map(|g| power::name_of(&g)).unwrap_or_default(),
        }

        // 2. pause services
        if self.elevated {
            for s in &d.profile.pause_services {
                if services::is_running(s) {
                    match services::stop(s) {
                        Ok(()) => a.stopped_services.push(s.clone()),
                        Err(e) => warnings.push(format!("Couldn't pause service {s}: {e}")),
                    }
                }
            }
        }

        // 3. timer resolution
        if d.profile.timer_resolution {
            a.timer_set = ntdll::request_timer_resolution(true);
        }

        self.active = Some(a);
        self.apply_target();
        self.prefer_dgpu_for_target();
        for w in warnings {
            self.event(EventKind::Warn, w);
        }

        let a = self.active.as_ref().expect("just set");
        let mut bits = Vec::new();
        if !a.plan_name.is_empty() && a.plan_applied.is_some() {
            bits.push(format!("power: {}", a.plan_name));
        }
        if !a.stopped_services.is_empty() {
            bits.push(format!("paused {}", a.stopped_services.join(", ")));
        }
        let target = if a.target_name.is_empty() { "system".to_string() } else { a.target_name.clone() };
        let msg = format!(
            "Boost ON - {} for {} ({}){}",
            a.profile.name,
            target,
            a.reason,
            if bits.is_empty() { String::new() } else { format!(" - {}", bits.join(" - ")) }
        );
        let balloon = (format!("Boosting: {}", a.profile.name), format!("{target} - background apps throttled"));
        self.event(EventKind::Boost, msg);
        if self.cfg.notifications {
            self.balloon = Some(balloon);
        }
        self.write_journal();
    }

    /// Laptops with two GPUs: set Windows' graphics preference so the boosted game runs on the
    /// dedicated GPU (takes effect the next time the game starts). Never overrides a user choice.
    fn prefer_dgpu_for_target(&mut self) {
        if !self.cfg.prefer_dgpu || !self.hybrid_gpu {
            return;
        }
        let Some((pid, name)) = self.active.as_ref().filter(|a| a.target_pid != 0).map(|a| (a.target_pid, a.target_name.clone()))
        else {
            return;
        };
        let lc = name.to_lowercase();
        let path = if config::contains(&self.cfg.anticheat_protected, &lc) {
            // anti-cheat games: no handle to the game, take its path from Windows' game list
            games::windows_known().into_iter().find(|g| g.name_lc == lc).map(|g| g.path_lc)
        } else {
            process::image_path(pid)
        };
        if let Some(p) = path {
            if crate::tweaks::gpu_pref::prefer_dedicated(&p) {
                self.event(
                    EventKind::Info,
                    format!("{name} is set to run on the dedicated GPU from its next launch (Windows graphics setting)"),
                );
            }
        }
    }

    /// Apply target settings (priority + no EcoQoS) unless it's anti-cheat protected.
    fn apply_target(&mut self) {
        let Some(mut a) = self.active.take() else { return };
        if a.target_pid != 0 {
            if let Some(t) = a.touched.remove(&a.target_pid) {
                t.restore();
            }
            let lc = a.target_name.to_lowercase();
            a.protected_target = config::contains(&self.cfg.anticheat_protected, &lc);
            if a.protected_target {
                if self.warned_protected.insert(lc) {
                    self.event(
                        EventKind::Info,
                        format!(
                            "{} uses kernel anti-cheat - Avix won't touch the game process itself, only everything around it. Tip: set its launch priority in Profiles.",
                            a.target_name
                        ),
                    );
                }
            } else {
                let class = match a.profile.priority {
                    TargetPriority::Keep => None,
                    TargetPriority::AboveNormal => Some(process::PRIO_ABOVE_NORMAL),
                    TargetPriority::High => Some(process::PRIO_HIGH),
                };
                a.target_touched = raise_process(a.target_pid, &a.target_name, class, Role::Target);
                if a.target_touched.is_none() {
                    self.event(EventKind::Warn, format!("Couldn't adjust {} (access denied)", a.target_name));
                }
            }
        }
        a.journal_dirty = true;
        self.active = Some(a);
    }

    fn retarget(&mut self, d: Desired) {
        if let Some(a) = self.active.as_mut() {
            if let Some(t) = a.target_touched.take() {
                t.restore();
            }
            a.target_pid = d.pid;
            a.target_name = d.name.clone();
            a.target_create = d.create;
            a.reason = d.reason;
            a.from_fullscreen = d.from_fullscreen;
            a.protected_target = false;
        }
        self.apply_target();
        self.prefer_dgpu_for_target();
        if !d.name.is_empty() {
            self.event(EventKind::Boost, format!("Boost target -> {}", d.name));
        }
    }

    fn maintain(&mut self) {
        let Some(mut a) = self.active.take() else { return };
        let mut events: Vec<(EventKind, String)> = Vec::new();
        let alive: HashSet<u32> = self.procs.iter().map(|p| p.pid).collect();
        let before = a.touched.len();
        a.touched.retain(|pid, _| alive.contains(pid));
        a.skip.retain(|pid| alive.contains(pid));
        if a.touched.len() != before {
            a.journal_dirty = true;
        }

        // Foreground exemption: whatever you're looking at is never throttled.
        let fg = self.fg_pid;
        if a.fg_exempt != Some(fg) {
            if let Some(old) = a.fg_exempt.take() {
                a.skip.remove(&old);
            }
            if fg != 0 && fg != a.target_pid {
                if let Some(t) = a.touched.get(&fg).filter(|t| t.role == Role::Background).cloned() {
                    t.restore();
                    a.touched.remove(&fg);
                    a.journal_dirty = true;
                }
                if !a.touched.contains_key(&fg) {
                    a.fg_exempt = Some(fg);
                    a.skip.insert(fg);
                }
            }
        }

        let cfg = &self.cfg;
        let mut to_close = Vec::new();
        for p in &self.procs {
            if p.pid <= 4
                || p.pid == self.own_pid
                || p.pid == a.target_pid
                || a.touched.contains_key(&p.pid)
                || a.skip.contains(&p.pid)
            {
                continue;
            }
            let lc = &p.name_lc;
            if config::contains(&cfg.never_touch, lc) || config::contains(&cfg.anticheat_protected, lc) {
                a.skip.insert(p.pid);
                continue;
            }
            if p.session == self.own_session && config::contains(&a.profile.close_apps, lc) {
                to_close.push((p.pid, p.name.clone()));
                a.skip.insert(p.pid);
                continue;
            }
            if config::contains(&a.profile.companions, lc) {
                match raise_process(p.pid, &p.name, Some(process::PRIO_ABOVE_NORMAL), Role::Companion) {
                    Some(t) => {
                        a.touched.insert(p.pid, t);
                        a.journal_dirty = true;
                    }
                    None => {
                        a.skip.insert(p.pid);
                    }
                }
                continue;
            }
            let eligible = a.profile.throttle_background
                && !config::contains(&a.profile.triggers, lc)
                && !self.games.contains_key(&p.pid)
                && !(a.target_pid != 0 && p.ppid == a.target_pid)
                && (p.session == self.own_session || config::contains(&cfg.background_hogs, lc))
                && (8..=10).contains(&p.base_priority);
            if eligible {
                if let Some(t) = throttle_process(p, &a.profile) {
                    a.touched.insert(p.pid, t);
                    a.journal_dirty = true;
                    continue;
                }
            }
            a.skip.insert(p.pid);
        }
        for (pid, name) in to_close {
            if process::kill(pid) {
                a.closed += 1;
                events.push((EventKind::Info, format!("Closed {name}")));
            }
        }

        // Every ~10 s: keep services paused and watch memory.
        let every = (10_000 / cfg.poll_interval_ms.max(500)).max(1);
        if self.tick_n % every == 0
            && a.profile.trim_memory
            && self.last_trim.map_or(true, |t| t.elapsed() >= TRIM_COOLDOWN)
        {
            let m = memory::status();
            if m.load >= 80 || m.avail < 1536 * 1024 * 1024 {
                let mut n = 0;
                for (pid, t) in &a.touched {
                    if t.role == Role::Background && process::trim_working_set(*pid) {
                        n += 1;
                    }
                }
                self.last_trim = Some(Instant::now());
                let freed = memory::status().avail.saturating_sub(m.avail);
                if n > 0 && freed > 64 * 1024 * 1024 {
                    events.push((
                        EventKind::Info,
                        format!("RAM was {}% full - freed {} from {n} background apps for the game", m.load, sys::fmt_bytes(freed)),
                    ));
                }
            }
        }
        if self.tick_n % every == 0 && self.elevated {
            for s in &a.profile.pause_services {
                if services::is_running(s) && services::stop(s).is_ok() && !a.stopped_services.contains(s) {
                    a.stopped_services.push(s.clone());
                    a.journal_dirty = true;
                }
            }
            if a.profile.standby_cleaner && self.last_purge.map_or(true, |t| t.elapsed() > PURGE_COOLDOWN) {
                if let Some((standby, free)) = memory::lists() {
                    let mb = 1024 * 1024;
                    if standby >= cfg.standby_min_mb * mb && free < cfg.free_memory_below_mb * mb {
                        if memory::purge_standby().is_ok() {
                            a.purges += 1;
                            self.last_purge = Some(Instant::now());
                            events.push((
                                EventKind::Info,
                                format!("Freed {} of standby memory (free was {})", sys::fmt_bytes(standby), sys::fmt_bytes(free)),
                            ));
                        }
                    }
                }
            }
        }

        let dirty = std::mem::take(&mut a.journal_dirty);
        self.active = Some(a);
        for (k, t) in events {
            self.event(k, t);
        }
        if dirty {
            self.write_journal();
        }
    }

    fn deactivate(&mut self) {
        let Some(a) = self.active.take() else { return };
        let mut restored = 0;
        for t in a.touched.values() {
            if t.restore() {
                restored += 1;
            }
        }
        if let Some(t) = &a.target_touched {
            t.restore();
        }
        let mut warnings = Vec::new();
        for s in &a.stopped_services {
            if let Err(e) = services::start(s) {
                warnings.push(format!("Couldn't restart service {s}: {e}"));
            }
        }
        if let (Some(prev), Some(applied)) = (a.prev_plan, a.plan_applied) {
            // only switch back if the user didn't pick another plan meanwhile
            if power::active().is_some_and(|cur| power::same(&cur, &applied)) {
                if let Err(e) = power::set_active(&prev) {
                    warnings.push(format!("Couldn't restore power plan: {e}"));
                }
            }
        }
        if a.timer_set {
            ntdll::request_timer_resolution(false);
        }
        journal::clear();
        let mins = a.since.elapsed().as_secs() / 60;
        self.event(
            EventKind::Info,
            format!("Boost OFF - {} restored after {mins} min ({restored} apps back to normal)", a.profile.name),
        );
        for w in warnings {
            self.event(EventKind::Warn, w);
        }
        if self.cfg.notifications {
            self.balloon = Some(("Boost ended".into(), format!("{} - everything restored", a.profile.name)));
        }
    }

    fn write_journal(&self) {
        let Some(a) = &self.active else { return };
        let mut touched: Vec<Touched> = a.touched.values().cloned().collect();
        if let Some(t) = &a.target_touched {
            touched.push(t.clone());
        }
        journal::write(&journal::Journal {
            prev_plan: a.prev_plan.as_ref().map(power::to_string),
            plan_applied: a.plan_applied.as_ref().map(power::to_string),
            stopped_services: a.stopped_services.clone(),
            touched,
            timer: a.timer_set,
        });
    }

    pub fn shutdown(&mut self) {
        if self.shut_down {
            return;
        }
        self.deactivate();
        self.shut_down = true;
        if let Some(mut c) = self.ui_child.take() {
            let _ = c.kill();
        }
    }

    // -------------------------------------------------------- commands

    pub fn handle_commands(&mut self) -> bool {
        let cmds = std::mem::take(&mut *lock(&self.shared.commands));
        let mut exit = false;
        for c in cmds {
            match c {
                Command::SetMode(m, p) => self.set_mode(m, p),
                Command::ReloadConfig => self.reload_config(),
                Command::PurgeStandby => self.purge_now(),
                Command::OpenUi => self.open_ui(),
                Command::Shutdown => exit = true,
                Command::CheckUpdate => {
                    self.update_force = true;
                    self.tick();
                }
            }
        }
        exit
    }

    pub fn set_mode(&mut self, mode: Mode, profile: Option<String>) {
        if self.mode == mode && self.forced_profile == profile {
            return;
        }
        self.mode = mode;
        self.forced_profile = profile;
        let text = match mode {
            Mode::Auto => "Mode: Auto-boost".to_string(),
            Mode::On => format!("Mode: Boost now ({})", self.forced_profile.clone().unwrap_or_else(|| "default profile".into())),
            Mode::Off => "Mode: Paused - Avix won't boost until you resume".to_string(),
        };
        self.event(EventKind::Info, text);
        self.last_tick = None;
        self.tick();
    }

    pub fn toggle_boost(&mut self) {
        if self.mode == Mode::On {
            self.set_mode(Mode::Auto, None);
        } else {
            self.set_mode(Mode::On, None);
        }
    }

    fn reload_config(&mut self) {
        self.cfg = Config::load();
        self.lists_at = None;
        self.hybrid_gpu = sys::hardware::is_hybrid(&sys::hardware::gpus());
        if !self.cfg.prefer_dgpu {
            let n = crate::tweaks::gpu_pref::remove_all();
            if n > 0 {
                self.event(EventKind::Info, format!("Removed the dedicated-GPU setting Avix made for {n} game(s)"));
            }
        }
        self.deactivate();
        self.event(EventKind::Info, "Settings reloaded".into());
        self.tick();
    }

    pub fn purge_now(&mut self) {
        let before = memory::lists();
        match memory::purge_standby() {
            Ok(()) => {
                self.last_purge = Some(Instant::now());
                if let Some(a) = self.active.as_mut() {
                    a.purges += 1;
                }
                let freed = before.map(|(s, _)| sys::fmt_bytes(s)).unwrap_or_else(|| "standby".into());
                self.event(EventKind::Info, format!("Freed {freed} of standby memory"));
            }
            Err(e) => self.event(EventKind::Warn, format!("Standby purge failed: {e}")),
        }
        self.publish();
    }

    pub fn open_ui(&mut self) {
        if let Some(c) = self.ui_child.as_mut() {
            if let Ok(None) = c.try_wait() {
                window::bring_to_front(UI_TITLE);
                return;
            }
        }
        if window::bring_to_front(UI_TITLE) {
            return;
        }
        self.ui_child = spawn_ui();
    }

    // -------------------------------------------------------- status

    fn publish(&mut self) {
        let mem = memory::status();
        if self.elevated && (self.lists.is_none() || self.tick_n % 3 == 0) {
            self.lists = memory::lists();
        }
        let (ws, cpu_time) = process::own_usage();
        // Report private working set (what Task Manager's "Memory" column shows).
        let ram = self.procs.iter().find(|p| p.pid == self.own_pid).map(|p| p.private_ws).filter(|&v| v > 0).unwrap_or(ws);
        let now = Instant::now();
        if let Some((prev, at)) = self.own_cpu {
            let secs = now.duration_since(at).as_secs_f64();
            if secs > 0.0 {
                self.own_cpu_percent =
                    (cpu_time.saturating_sub(prev) as f64 / 1e7 / secs / process::logical_cpus() as f64 * 100.0) as f32;
            }
        }
        self.own_cpu = Some((cpu_time, now));

        let active = self.active.as_ref().map(|a| ActiveInfo {
            profile: a.profile.name.clone(),
            target: a.target_name.clone(),
            target_pid: a.target_pid,
            reason: a.reason.clone(),
            since_secs: a.since.elapsed().as_secs(),
            power_plan: a.plan_name.clone(),
            throttled: a.touched.values().filter(|t| t.role == Role::Background).count() as u32,
            raised: a.touched.values().filter(|t| t.role == Role::Companion).count() as u32
                + a.target_touched.is_some() as u32,
            services_paused: a.stopped_services.clone(),
            apps_closed: a.closed,
            standby_purges: a.purges,
            timer_resolution: a.timer_set,
            protected_target: a.protected_target,
        });
        let st = Status {
            version: crate::VERSION.to_string(),
            elevated: self.elevated,
            mode: self.mode,
            forced_profile: self.forced_profile.clone(),
            active,
            foreground: self.fg_name.clone(),
            cpu_percent: self.cpu_percent,
            mem_used: mem.total.saturating_sub(mem.avail),
            mem_total: mem.total,
            standby: self.lists.map(|l| l.0),
            free: self.lists.map(|l| l.1),
            engine_ram: ram,
            engine_cpu_percent: self.own_cpu_percent,
            uptime_secs: self.started.elapsed().as_secs(),
            detected_games: self.detected.clone(),
            events: self.events.iter().cloned().collect(),
            update: self.updater.status(),
        };
        *lock(&self.shared.status) = st;
    }

    // -------------------------------------------------------- tray helpers

    pub fn icon_state(&self) -> IconState {
        match (self.mode, &self.active) {
            (_, Some(_)) => IconState::Boosting,
            (Mode::Off, None) => IconState::Paused,
            _ => IconState::Idle,
        }
    }

    pub fn status_line(&self) -> String {
        match (&self.active, self.mode) {
            (Some(a), _) if !a.target_name.is_empty() => format!("Boosting {} - {}", a.profile.name, a.target_name),
            (Some(a), _) => format!("Boosting {}", a.profile.name),
            (None, Mode::Off) => "Paused".into(),
            (None, _) => "Watching for games & heavy apps".into(),
        }
    }

    pub fn tooltip(&self) -> String {
        let mut t = format!("Avix - {}", self.status_line());
        if !self.elevated {
            t.push_str(" (limited)");
        }
        t
    }

    pub fn take_balloon(&mut self) -> Option<(String, String)> {
        self.balloon.take()
    }

    pub fn mode(&self) -> Mode {
        self.mode
    }

    pub fn forced_profile(&self) -> Option<String> {
        self.forced_profile.clone()
    }

    pub fn profile_names(&self) -> Vec<String> {
        self.cfg.profiles.iter().filter(|p| p.enabled).map(|p| p.name.clone()).collect()
    }
}
