//! A stand-in "game" for demos and the synthetic benchmark. Not part of Avix.exe.
//!
//! Avix treats it as a game when the exe runs from a folder named `Games` and shows a window
//! (the same rule as any game installed to D:\Games). Build and copy it there:
//!
//!   cargo build --release --locked --example demo_game
//!   copy target\release\examples\demo_game.exe <somewhere>\Games\avix-demo-game.exe
//!
//! Modes:
//!   avix-demo-game.exe [--pos X,Y]               a window with a moving scene (for demos)
//!   avix-demo-game.exe --calibrate 4             print the work units that take ~4 ms per frame
//!   avix-demo-game.exe --load 16                 no window: 16 busy threads (background load)
//!   avix-demo-game.exe --bench 30 --warmup 6 --work N --out result.json [--pos X,Y]
//!       uncapped frames, each doing N work units on the main thread (like a CPU-bound game);
//!       records frame times after the warm-up, writes average FPS and 1% / 0.1% lows, exits.

use eframe::egui::{self, pos2, vec2, Color32, Rect, Stroke};
use std::hint::black_box;
use std::time::{Duration, Instant};

/// One unit of CPU work: a small integer hash loop the optimizer can't remove.
fn work(units: u64) -> u64 {
    let mut x = 0x9E37_79B9_7F4A_7C15u64;
    for i in 0..units * 1000 {
        x ^= x << 13;
        x ^= x >> 7;
        x ^= x << 17;
        x = x.wrapping_add(black_box(i));
    }
    black_box(x)
}

fn arg(name: &str) -> Option<String> {
    let args: Vec<String> = std::env::args().collect();
    args.iter().position(|a| a == name).and_then(|i| args.get(i + 1).cloned())
}

fn main() {
    if let Some(ms) = arg("--calibrate") {
        let ms: f64 = ms.parse().unwrap_or(4.0);
        work(200); // warm up the clock
        let t = Instant::now();
        let n = 2000;
        work(n);
        let per_unit = t.elapsed().as_secs_f64() * 1000.0 / n as f64;
        println!("{}", (ms / per_unit).round() as u64);
        return;
    }
    if let Some(threads) = arg("--load") {
        let n: usize = threads.parse().unwrap_or(4);
        let handles: Vec<_> = (0..n).map(|_| std::thread::spawn(|| loop { work(1000); })).collect();
        for h in handles {
            let _ = h.join();
        }
        return;
    }

    let bench = arg("--bench").and_then(|s| s.parse::<f64>().ok());
    let pos = arg("--pos").and_then(|p| {
        let (x, y) = p.split_once(',')?;
        Some([x.trim().parse::<f32>().ok()?, y.trim().parse::<f32>().ok()?])
    });
    let mut viewport = egui::ViewportBuilder::default().with_title("avix-demo-game").with_inner_size([640.0, 480.0]);
    if let Some(p) = pos {
        viewport = viewport.with_position(p);
    }
    let options = eframe::NativeOptions { viewport, vsync: bench.is_none(), centered: pos.is_none(), ..Default::default() };
    let app = Game {
        start: Instant::now(),
        last: None,
        bench: bench.map(|secs| Bench {
            secs,
            warmup: arg("--warmup").and_then(|s| s.parse().ok()).unwrap_or(6.0),
            units: arg("--work").and_then(|s| s.parse().ok()).unwrap_or(400),
            out: arg("--out").unwrap_or_else(|| "demo-game-result.json".into()),
            frames: Vec::with_capacity(100_000),
            done: false,
        }),
        fps: 0.0,
    };
    let _ = eframe::run_native("avix-demo-game", options, Box::new(|_| Ok(Box::new(app))));
}

struct Bench {
    secs: f64,
    warmup: f64,
    units: u64,
    out: String,
    /// Frame times in ms, recorded after the warm-up.
    frames: Vec<f64>,
    done: bool,
}

struct Game {
    start: Instant,
    last: Option<Instant>,
    bench: Option<Bench>,
    fps: f64,
}

impl Bench {
    fn write(&self) {
        let mut sorted = self.frames.clone();
        sorted.sort_by(|a, b| b.total_cmp(a)); // slowest first
        let total_ms: f64 = sorted.iter().sum();
        let low = |share: f64| {
            let n = ((sorted.len() as f64 * share).ceil() as usize).max(1).min(sorted.len());
            1000.0 / (sorted[..n].iter().sum::<f64>() / n as f64)
        };
        let result = serde_json::json!({
            "frames": sorted.len(),
            "seconds": total_ms / 1000.0,
            "work_units": self.units,
            "avg_fps": sorted.len() as f64 * 1000.0 / total_ms,
            "low_1pct_fps": low(0.01),
            "low_01pct_fps": low(0.001),
            "worst_frame_ms": sorted.first().copied().unwrap_or(0.0),
        });
        let _ = std::fs::write(&self.out, serde_json::to_string_pretty(&result).unwrap_or_default());
    }
}

impl eframe::App for Game {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        let now = Instant::now();
        if let Some(last) = self.last {
            let dt = now.duration_since(last).as_secs_f64();
            self.fps = self.fps * 0.95 + (1.0 / dt.max(1e-6)) * 0.05;
            let t = now.duration_since(self.start).as_secs_f64();
            if let Some(b) = &mut self.bench {
                if !b.done && t >= b.warmup {
                    b.frames.push(dt * 1000.0);
                    if t >= b.warmup + b.secs {
                        b.done = true;
                        b.write();
                        ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                    }
                }
            }
        }
        self.last = Some(now);
        if let Some(b) = &self.bench {
            work(b.units); // the CPU-bound part of the frame
        }

        egui::CentralPanel::default().frame(egui::Frame::new().fill(Color32::from_rgb(11, 11, 14))).show(ctx, |ui| {
            let r = ui.max_rect();
            let p = ui.painter();
            let t = self.start.elapsed().as_secs_f32();
            for i in 0..14 {
                let f = i as f32 / 14.0;
                let x = r.left() + (f * r.width() + t * 90.0 * (0.4 + f)) % r.width();
                let y = r.center().y + (t * 1.7 + f * 6.28).sin() * r.height() * 0.3;
                let c = Color32::from_rgb((92.0 + 120.0 * f) as u8, 92, 255);
                p.rect_filled(Rect::from_center_size(pos2(x, y), vec2(28.0, 28.0)), 6.0, c);
            }
            p.rect_stroke(r.shrink(8.0), 10.0, Stroke::new(1.0_f32, Color32::from_gray(52)), egui::StrokeKind::Inside);
            let label = if self.bench.is_some() { "avix-demo-game · synthetic benchmark" } else { "avix-demo-game · stand-in test game" };
            p.text(r.left_top() + vec2(20.0, 18.0), egui::Align2::LEFT_TOP, label, egui::FontId::proportional(16.0), Color32::from_gray(230));
            p.text(r.left_top() + vec2(20.0, 42.0), egui::Align2::LEFT_TOP, format!("{:.0} FPS", self.fps), egui::FontId::monospace(14.0), Color32::from_gray(160));
        });
        ctx.request_repaint();
        if self.bench.is_none() {
            std::thread::sleep(Duration::from_millis(1)); // demo mode: no need to spin
        }
    }
}
