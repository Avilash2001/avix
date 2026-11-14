//! Debug builds only: renders pages to PNG for review and the README.
//!
//!   AVIX_SHOT=<dir>                 turn it on; files land in <dir> and the window closes after
//!   AVIX_SHOT_PAGES=dashboard,...   pages to capture (default: all); also the scenes search,
//!                                   confirm, welcome and savebar (overlays, with toasts), and
//!                                   about (Settings scrolled to its About card)
//!   AVIX_SHOT_THEMES=light,dark     themes to capture (default: both)
//!   AVIX_SHOT_SIZE=1280x800         window size
//!   AVIX_SHOT_SETTLE=3              seconds to wait on each page before capturing
//!
//! It also writes the app icon (all three tray states) as PNGs next to the shots.
//!
//! Recording (for the README demo): `AVIX_RECORD=<file.mp4>` streams the dashboard's own frames
//! to ffmpeg. Only Avix's window content is captured (egui screenshots of its own frame), never
//! the screen, so nothing else on the desktop can end up in the video.
//!   AVIX_RECORD_FPS=15            frames per second
//!   AVIX_RECORD_SECS=60           stop after this long at most (or when `<file>.stop` appears)
//!   AVIX_RECORD_FFMPEG=<path>     ffmpeg to encode with (default: ffmpeg on PATH)

use super::{App, Page};
use crate::config::UiTheme;
use crate::icon_render::{render_rgba, IconState};
use eframe::egui;
use std::path::PathBuf;

pub fn window_size() -> Option<[f32; 2]> {
    let v = std::env::var("AVIX_SHOT_SIZE").ok()?;
    let (w, h) = v.split_once('x')?;
    Some([w.trim().parse().ok()?, h.trim().parse().ok()?])
}

#[derive(Clone, Copy, Debug)]
enum Scene {
    Page(Page),
    Search,
    Confirm,
    Welcome,
    Savebar,
    About,
}

impl Scene {
    fn name(self) -> String {
        match self {
            Scene::Page(p) => format!("{p:?}").to_lowercase(),
            s => format!("{s:?}").to_lowercase(),
        }
    }
    fn page(self) -> Page {
        match self {
            Scene::Page(p) => p,
            Scene::Search | Scene::Welcome => Page::Dashboard,
            Scene::Confirm => Page::Debloat,
            Scene::Savebar | Scene::About => Page::Settings,
        }
    }
}

pub struct Shot {
    dir: PathBuf,
    queue: Vec<(Scene, UiTheme)>,
    idx: usize,
    t0: Option<f64>,
    requested: bool,
    settle: f64,
}

impl Shot {
    pub fn from_env() -> Option<Self> {
        let dir = PathBuf::from(std::env::var_os("AVIX_SHOT")?);
        let _ = std::fs::create_dir_all(&dir);
        let pages: Vec<Scene> = match std::env::var("AVIX_SHOT_PAGES") {
            Ok(v) => v
                .split(',')
                .filter_map(|n| {
                    let n = n.trim();
                    Page::ALL.into_iter().find(|p| format!("{p:?}").eq_ignore_ascii_case(n)).map(Scene::Page).or(match n {
                        "search" => Some(Scene::Search),
                        "confirm" => Some(Scene::Confirm),
                        "welcome" => Some(Scene::Welcome),
                        "savebar" => Some(Scene::Savebar),
                        "about" => Some(Scene::About),
                        _ => None,
                    })
                })
                .collect(),
            Err(_) => Page::ALL.into_iter().map(Scene::Page).collect(),
        };
        let themes: Vec<UiTheme> = match std::env::var("AVIX_SHOT_THEMES") {
            Ok(v) => v
                .split(',')
                .filter_map(|n| match n.trim() {
                    "light" => Some(UiTheme::Light),
                    "dark" => Some(UiTheme::Dark),
                    _ => None,
                })
                .collect(),
            Err(_) => vec![UiTheme::Light, UiTheme::Dark],
        };
        let settle = std::env::var("AVIX_SHOT_SETTLE").ok().and_then(|v| v.parse().ok()).unwrap_or(3.0);
        let queue = themes.iter().flat_map(|t| pages.iter().map(move |p| (*p, *t))).collect();
        Some(Self { dir, queue, idx: 0, t0: None, requested: false, settle })
    }
}

pub fn step(app: &mut App, ctx: &egui::Context) {
    let Some(mut shot) = app.shot.take() else { return };
    ctx.request_repaint();
    let now = ctx.input(|i| i.time);
    let shots: Vec<std::sync::Arc<egui::ColorImage>> = ctx.input(|i| {
        i.raw
            .events
            .iter()
            .filter_map(|e| match e {
                egui::Event::Screenshot { image, .. } => Some(image.clone()),
                _ => None,
            })
            .collect()
    });
    for img in shots {
        if let Some((scene, theme)) = shot.queue.get(shot.idx).copied() {
            let name = format!("{}-{}.png", scene.name(), theme.label().to_lowercase());
            let rgba: Vec<u8> = img.pixels.iter().flat_map(|c| c.to_srgba_unmultiplied()).collect();
            let _ = std::fs::write(shot.dir.join(name), png(img.size[0] as u32, img.size[1] as u32, &rgba));
        }
        // undo the scene's setup
        app.confirm = None;
        app.toasts.clear();
        app.search.text.clear();
        app.cfg_saved.welcomed = true;
        app.cfg = app.cfg_saved.clone();
        shot.idx += 1;
        shot.t0 = None;
        shot.requested = false;
    }
    if shot.idx >= shot.queue.len() {
        for (s, n) in [(IconState::Idle, "idle"), (IconState::Boosting, "boosting"), (IconState::Paused, "paused")] {
            for size in [16u32, 32, 64, 256] {
                let _ = std::fs::write(shot.dir.join(format!("icon-{n}-{size}.png")), png(size, size, &render_rgba(size, s)));
            }
        }
        ctx.send_viewport_cmd(egui::ViewportCommand::Close);
        app.shot = Some(shot);
        return;
    }
    let (scene, theme) = shot.queue[shot.idx];
    if shot.t0.is_none() {
        app.page = scene.page();
        app.page_since = -10.0;
        super::apply_theme(ctx, theme);
        shot.t0 = Some(now);
        match scene {
            Scene::Search => app.search.text = "game".into(),
            Scene::Confirm => app.ask(
                "Apply recommended tweaks?",
                "Avix will apply 25 low-risk tweaks (Game DVR off, telemetry off, Start ads off, gaming scheduler tweaks...). Each one can be reverted individually. Some need a reboot to fully kick in.",
                "Apply",
                false,
                |_| {},
            ),
            Scene::Welcome => app.cfg_saved.welcomed = false,
            Scene::Savebar => {
                app.cfg.hotkey = !app.cfg.hotkey;
                app.toast(super::ToastKind::Ok, "Applied: Turn off Game DVR background recording");
                app.toast(super::ToastKind::Err, "Couldn't end MsMpEng.exe: Windows protects it, or Avix needs admin rights");
            }
            Scene::About => app.settings.focus = Some(("about", now)),
            Scene::Page(_) => {}
        }
    }
    if matches!(scene, Scene::Search) {
        if let Some(id) = app.search.field_id {
            ctx.memory_mut(|m| m.request_focus(id));
        }
    }
    if !shot.requested && now - shot.t0.unwrap_or(now) > shot.settle {
        ctx.send_viewport_cmd(egui::ViewportCommand::Screenshot(egui::UserData::default()));
        shot.requested = true;
    }
    app.shot = Some(shot);
}

// ------------------------------------------------------------------ recording

pub struct Recorder {
    out: PathBuf,
    ffmpeg: String,
    every: f64,
    secs: f64,
    started: Option<f64>,
    next: f64,
    pending: bool,
    /// Frames go to a writer thread, so encoding never stalls the dashboard.
    tx: Option<std::sync::mpsc::SyncSender<std::sync::Arc<egui::ColorImage>>>,
    writer: Option<std::thread::JoinHandle<()>>,
    size: [usize; 2],
    frames: u64,
    dropped: u64,
    /// Dashboard frames seen while recording (to tell slow screenshots from slow frames).
    updates: u64,
    done: bool,
}

impl Recorder {
    pub fn from_env() -> Option<Self> {
        let out = PathBuf::from(std::env::var_os("AVIX_RECORD")?);
        let num = |k: &str, d: f64| std::env::var(k).ok().and_then(|v| v.parse::<f64>().ok()).unwrap_or(d);
        Some(Self {
            out,
            ffmpeg: std::env::var("AVIX_RECORD_FFMPEG").unwrap_or_else(|_| "ffmpeg".into()),
            every: 1.0 / num("AVIX_RECORD_FPS", 15.0).clamp(1.0, 60.0),
            secs: num("AVIX_RECORD_SECS", 60.0),
            started: None,
            next: 0.0,
            pending: false,
            tx: None,
            writer: None,
            size: [0, 0],
            frames: 0,
            dropped: 0,
            updates: 0,
            done: false,
        })
    }

    fn stop_requested(&self) -> bool {
        let mut p = self.out.as_os_str().to_owned();
        p.push(".stop");
        PathBuf::from(p).exists()
    }

    fn start_encoder(&mut self, w: usize, h: usize) -> bool {
        use std::io::Write;
        let fps = format!("{}", (1.0 / self.every).round());
        let spawned = crate::sys::hidden_cmd(&self.ffmpeg)
            .args(["-y", "-loglevel", "error", "-f", "rawvideo", "-pix_fmt", "rgba", "-s", &format!("{w}x{h}"), "-r", &fps, "-i", "-"])
            .args(["-vf", "pad=ceil(iw/2)*2:ceil(ih/2)*2", "-c:v", "libx264", "-preset", "ultrafast", "-crf", "14", "-pix_fmt", "yuv420p"])
            .arg(&self.out)
            .stdin(std::process::Stdio::piped())
            .spawn();
        let mut child = match spawned {
            Ok(c) => c,
            Err(e) => {
                crate::logging::log(&format!("Recording: couldn't start ffmpeg ({e})"));
                return false;
            }
        };
        let Some(mut stdin) = child.stdin.take() else { return false };
        let (tx, rx) = std::sync::mpsc::sync_channel::<std::sync::Arc<egui::ColorImage>>(16);
        self.writer = Some(std::thread::spawn(move || {
            for img in rx {
                // Color32 is four bytes in RGBA order, and a screenshot is opaque, so the
                // pixels are already raw RGBA: write them without copying.
                let px = &img.pixels;
                let bytes = unsafe { std::slice::from_raw_parts(px.as_ptr().cast::<u8>(), px.len() * 4) };
                if stdin.write_all(bytes).is_err() {
                    break;
                }
            }
            drop(stdin);
            let _ = child.wait();
        }));
        self.tx = Some(tx);
        self.size = [w, h];
        true
    }

    fn finish(&mut self) {
        drop(self.tx.take());
        if let Some(w) = self.writer.take() {
            let _ = w.join();
        }
        self.done = true;
        crate::logging::log(&format!(
            "Recording finished: {} frames ({} dropped) from {} dashboard frames -> {}",
            self.frames,
            self.dropped,
            self.updates,
            self.out.display()
        ));
    }
}

fn screenshots(ctx: &egui::Context) -> Vec<std::sync::Arc<egui::ColorImage>> {
    ctx.input(|i| {
        i.raw
            .events
            .iter()
            .filter_map(|e| match e {
                egui::Event::Screenshot { image, .. } => Some(image.clone()),
                _ => None,
            })
            .collect()
    })
}

pub fn record(app: &mut App, ctx: &egui::Context) {
    let Some(rec) = app.rec.as_mut() else { return };
    if rec.done {
        return;
    }
    ctx.request_repaint();
    rec.updates += 1;
    let now = ctx.input(|i| i.time);
    let start = *rec.started.get_or_insert(now);
    for img in screenshots(ctx) {
        rec.pending = false;
        let [w, h] = img.size;
        if rec.tx.is_none() && !rec.start_encoder(w, h) {
            rec.done = true;
            return;
        }
        if [w, h] != rec.size {
            continue; // the window was resized mid-recording: keep the video's frame size
        }
        match rec.tx.as_ref().map(|tx| tx.try_send(img)) {
            Some(Ok(())) => rec.frames += 1,
            Some(Err(std::sync::mpsc::TrySendError::Full(_))) => rec.dropped += 1,
            _ => {
                rec.finish();
                return;
            }
        }
    }
    if now - start >= rec.secs || rec.stop_requested() {
        rec.finish();
        return;
    }
    // egui_glow panics reading a zero-sized frame: wait until the window is really there
    let size = ctx.screen_rect().size();
    if !rec.pending && now >= rec.next && now - start > 0.5 && size.x >= 1.0 && size.y >= 1.0 {
        ctx.send_viewport_cmd(egui::ViewportCommand::Screenshot(egui::UserData::default()));
        rec.pending = true;
        // keep a steady rate; after a slow frame, don't burst to catch up
        rec.next = if rec.next == 0.0 || rec.next + rec.every < now { now + rec.every } else { rec.next + rec.every };
    }
}

// ------------------------------------------------------------------ minimal PNG writer

fn crc32(data: &[u8]) -> u32 {
    let mut table = [0u32; 256];
    for (i, t) in table.iter_mut().enumerate() {
        let mut c = i as u32;
        for _ in 0..8 {
            c = if c & 1 != 0 { 0xEDB8_8320 ^ (c >> 1) } else { c >> 1 };
        }
        *t = c;
    }
    let mut c = 0xFFFF_FFFFu32;
    for &b in data {
        c = table[((c ^ b as u32) & 0xFF) as usize] ^ (c >> 8);
    }
    c ^ 0xFFFF_FFFF
}

/// Uncompressed (stored-deflate) RGBA PNG. Big files, zero dependencies; fine for debug captures.
pub fn png(w: u32, h: u32, rgba: &[u8]) -> Vec<u8> {
    let mut raw = Vec::with_capacity((w * 4 + 1) as usize * h as usize);
    for y in 0..h as usize {
        raw.push(0);
        raw.extend_from_slice(&rgba[y * w as usize * 4..(y + 1) * w as usize * 4]);
    }
    let mut z = vec![0x78, 0x01];
    let chunks: Vec<&[u8]> = raw.chunks(65_535).collect();
    for (i, c) in chunks.iter().enumerate() {
        z.push(if i + 1 == chunks.len() { 1 } else { 0 });
        let len = c.len() as u16;
        z.extend_from_slice(&len.to_le_bytes());
        z.extend_from_slice(&(!len).to_le_bytes());
        z.extend_from_slice(c);
    }
    let (mut a, mut b) = (1u32, 0u32);
    for &x in &raw {
        a = (a + x as u32) % 65_521;
        b = (b + a) % 65_521;
    }
    z.extend_from_slice(&((b << 16) | a).to_be_bytes());

    let mut out = vec![0x89, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A];
    let mut chunk = |kind: &[u8; 4], data: &[u8]| {
        out.extend_from_slice(&(data.len() as u32).to_be_bytes());
        let mut body = kind.to_vec();
        body.extend_from_slice(data);
        out.extend_from_slice(&body);
        out.extend_from_slice(&crc32(&body).to_be_bytes());
    };
    let mut ihdr = Vec::new();
    ihdr.extend_from_slice(&w.to_be_bytes());
    ihdr.extend_from_slice(&h.to_be_bytes());
    ihdr.extend_from_slice(&[8, 6, 0, 0, 0]);
    chunk(b"IHDR", &ihdr);
    chunk(b"IDAT", &z);
    chunk(b"IEND", &[]);
    out
}
