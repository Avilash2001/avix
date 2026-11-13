//! Procedurally rendered app icon (rounded tile + lightning bolt), so the binary
//! needs no resource compiler and the tray icon can change colour with state.

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

/// Build a Win32 HICON (caller owns it; destroy with DestroyIcon).
pub fn make_hicon(size: u32, state: IconState) -> windows_sys::Win32::UI::WindowsAndMessaging::HICON {
    use windows_sys::Win32::Graphics::Gdi::{
        CreateBitmap, CreateDIBSection, DeleteObject, BITMAPINFO, BITMAPINFOHEADER, BI_RGB,
        DIB_RGB_COLORS,
    };
    use windows_sys::Win32::UI::WindowsAndMessaging::{CreateIconIndirect, ICONINFO};

    let rgba = render_rgba(size, state);
    unsafe {
        let mut bmi: BITMAPINFO = std::mem::zeroed();
        bmi.bmiHeader = BITMAPINFOHEADER {
            biSize: std::mem::size_of::<BITMAPINFOHEADER>() as u32,
            biWidth: size as i32,
            biHeight: -(size as i32),
            biPlanes: 1,
            biBitCount: 32,
            biCompression: BI_RGB,
            biSizeImage: 0,
            biXPelsPerMeter: 0,
            biYPelsPerMeter: 0,
            biClrUsed: 0,
            biClrImportant: 0,
        };
        let mut bits: *mut std::ffi::c_void = std::ptr::null_mut();
        let color = CreateDIBSection(std::ptr::null_mut(), &bmi, DIB_RGB_COLORS, &mut bits, std::ptr::null_mut(), 0);
        if color.is_null() || bits.is_null() {
            return std::ptr::null_mut();
        }
        let dst = std::slice::from_raw_parts_mut(bits as *mut u8, (size * size * 4) as usize);
        for (d, s) in dst.chunks_exact_mut(4).zip(rgba.chunks_exact(4)) {
            d[0] = s[2];
            d[1] = s[1];
            d[2] = s[0];
            d[3] = s[3];
        }
        let mask_bytes = vec![0u8; (size.div_ceil(16) * 2 * size) as usize];
        let mask = CreateBitmap(size as i32, size as i32, 1, 1, mask_bytes.as_ptr() as *const _);
        let info = ICONINFO { fIcon: 1, xHotspot: 0, yHotspot: 0, hbmMask: mask, hbmColor: color };
        let icon = CreateIconIndirect(&info);
        DeleteObject(color);
        DeleteObject(mask);
        icon
    }
}
