//! The app icon, drawn in code: a rounded squircle tile with a white mark - an upward chevron with
//! a dot under its apex, which reads as an "A" and as "boost". Pure Rust with no Windows calls, so
//! build.rs can also use it to embed the same icon in Avix.exe.
//!
//! Tray states change the tile colour: blue (watching), green (boosting), grey (paused). Below
//! 24 px the dot is dropped and the stroke thickened so the mark stays crisp.

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum IconState {
    Idle,
    Boosting,
    Paused,
}

impl IconState {
    /// Tile gradient, top then bottom.
    fn colors(self) -> ([f32; 3], [f32; 3]) {
        match self {
            IconState::Idle => ([92.0, 92.0, 255.0], [44.0, 44.0, 222.0]),
            IconState::Boosting => ([52.0, 208.0, 120.0], [17.0, 150.0, 74.0]),
            IconState::Paused => ([156.0, 159.0, 170.0], [108.0, 111.0, 122.0]),
        }
    }
}

/// Distance from p to the segment ab.
fn seg_dist(p: (f32, f32), a: (f32, f32), b: (f32, f32)) -> f32 {
    let (abx, aby) = (b.0 - a.0, b.1 - a.1);
    let (apx, apy) = (p.0 - a.0, p.1 - a.1);
    let t = ((apx * abx + apy * aby) / (abx * abx + aby * aby)).clamp(0.0, 1.0);
    let (dx, dy) = (apx - abx * t, apy - aby * t);
    (dx * dx + dy * dy).sqrt()
}

/// Straight-alpha RGBA pixels, row-major, top-down.
pub fn render_rgba(size: u32, state: IconState) -> Vec<u8> {
    let (top, bottom) = state.colors();
    let small = size <= 24;
    // full-bleed tile in the tray, a hair of margin for the app icon
    let half = if small { 0.5 } else { 0.47 };
    let n = 5.0f32; // squircle exponent
    let hw = if small { 0.095 } else { 0.074 }; // chevron half-width
    let apex = (0.5, if small { 0.25 } else { 0.27 });
    let (l, r) = if small { ((0.24, 0.72), (0.76, 0.72)) } else { ((0.275, 0.715), (0.725, 0.715)) };
    let dot = (0.5, 0.648, 0.066);

    let ss = 5u32;
    let mut out = vec![0u8; (size * size * 4) as usize];
    for py in 0..size {
        for px in 0..size {
            let (mut cov_tile, mut cov_mark) = (0u32, 0u32);
            for sy in 0..ss {
                for sx in 0..ss {
                    let x = (px as f32 + (sx as f32 + 0.5) / ss as f32) / size as f32;
                    let y = (py as f32 + (sy as f32 + 0.5) / ss as f32) / size as f32;
                    let (u, v) = (((x - 0.5) / half).abs(), ((y - 0.5) / half).abs());
                    if u.powf(n) + v.powf(n) > 1.0 {
                        continue;
                    }
                    cov_tile += 1;
                    let p = (x, y);
                    let on_chevron = seg_dist(p, l, apex) <= hw || seg_dist(p, apex, r) <= hw;
                    let on_dot = !small && ((x - dot.0).powi(2) + (y - dot.1).powi(2)).sqrt() <= dot.2;
                    if on_chevron || on_dot {
                        cov_mark += 1;
                    }
                }
            }
            if cov_tile == 0 {
                continue;
            }
            let a = cov_tile as f32 / (ss * ss) as f32;
            let t = (py as f32 / size as f32 - (0.5 - half)) / (2.0 * half);
            let t = t.clamp(0.0, 1.0);
            let w = cov_mark as f32 / cov_tile as f32;
            let i = ((py * size + px) * 4) as usize;
            for c in 0..3 {
                let bg = top[c] * (1.0 - t) + bottom[c] * t;
                out[i + c] = (bg * (1.0 - w) + 255.0 * w).round() as u8;
            }
            out[i + 3] = (a * 255.0).round() as u8;
        }
    }
    out
}
