//! The app icon (rounded tile + lightning bolt), drawn in code. Pure Rust with no Windows
//! calls, so build.rs can also use it to embed the same icon in Avix.exe.

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum IconState {
    Idle,
    Boosting,
    Paused,
}

impl IconState {
    fn colors(self) -> ([u8; 3], [u8; 3]) {
        match self {
            IconState::Idle => ([124, 92, 255], [79, 70, 229]),
            IconState::Boosting => ([52, 211, 153], [16, 160, 110]),
            IconState::Paused => ([120, 126, 140], [75, 80, 92]),
        }
    }
}

const BOLT: [(f32, f32); 7] = [
    (0.60, 0.10),
    (0.25, 0.56),
    (0.47, 0.56),
    (0.39, 0.90),
    (0.76, 0.42),
    (0.54, 0.42),
    (0.66, 0.10),
];

fn in_poly(x: f32, y: f32, poly: &[(f32, f32)]) -> bool {
    let mut inside = false;
    let mut j = poly.len() - 1;
    for i in 0..poly.len() {
        let (xi, yi) = poly[i];
        let (xj, yj) = poly[j];
        if (yi > y) != (yj > y) && x < (xj - xi) * (y - yi) / (yj - yi) + xi {
            inside = !inside;
        }
        j = i;
    }
    inside
}

fn in_rounded_rect(x: f32, y: f32, r: f32) -> bool {
    let cx = x.clamp(r, 1.0 - r);
    let cy = y.clamp(r, 1.0 - r);
    (x - cx).powi(2) + (y - cy).powi(2) <= r * r
}

/// Straight-alpha RGBA pixels, row-major, top-down.
pub fn render_rgba(size: u32, state: IconState) -> Vec<u8> {
    let (top, bottom) = state.colors();
    let ss = 4u32;
    let mut out = vec![0u8; (size * size * 4) as usize];
    for py in 0..size {
        for px in 0..size {
            let (mut cov_tile, mut cov_bolt) = (0u32, 0u32);
            for sy in 0..ss {
                for sx in 0..ss {
                    let x = (px as f32 + (sx as f32 + 0.5) / ss as f32) / size as f32;
                    let y = (py as f32 + (sy as f32 + 0.5) / ss as f32) / size as f32;
                    if in_rounded_rect(x, y, 0.24) {
                        cov_tile += 1;
                        if in_poly(x, y, &BOLT) {
                            cov_bolt += 1;
                        }
                    }
                }
            }
            let n = (ss * ss) as f32;
            let a = cov_tile as f32 / n;
            if a <= 0.0 {
                continue;
            }
            let t = py as f32 / size as f32;
            let bg = [0, 1, 2].map(|i| top[i] as f32 * (1.0 - t) + bottom[i] as f32 * t);
            let w = if cov_tile > 0 { cov_bolt as f32 / cov_tile as f32 } else { 0.0 };
            let i = ((py * size + px) * 4) as usize;
            for c in 0..3 {
                out[i + c] = (bg[c] * (1.0 - w) + 255.0 * w).round() as u8;
            }
            out[i + 3] = (a * 255.0).round() as u8;
        }
    }
    out
}
