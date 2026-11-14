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
