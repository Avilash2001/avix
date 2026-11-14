//! Avix look & feel: light and dark colour tokens, Geist type, Lucide icons, motion, and the small
//! painted widgets every page is built from. Pages only ever ask for tokens (`text()`, `accent()`,
//! `Tone::Good`...), so the theme switch and the animations setting apply everywhere at once.

use eframe::egui::{
    self, emath::easing, epaint::Mesh, pos2, vec2, Align, Align2, Color32, Context, CornerRadius,
    FontData, FontDefinitions, FontFamily, FontId, Frame, Id, Layout, Margin, Pos2, Rect, Response,
    RichText, Sense, Shadow, Stroke, StrokeKind, TextStyle, Theme, Ui, UiBuilder,
};
use std::hash::Hash;
use std::ops::RangeInclusive;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

// ================================================================== palette

#[derive(Clone, Copy)]
pub struct Palette {
    pub canvas: Color32,
    pub surface: Color32,
    pub sunken: Color32,
    pub hover: Color32,
    pub border: Color32,
    pub border_strong: Color32,
    pub text: Color32,
    pub muted: Color32,
    pub faint: Color32,
    pub accent: Color32,
    pub accent_hover: Color32,
    pub accent_soft: Color32,
    pub accent_text: Color32,
    pub selection: Color32,
    pub good: Color32,
    pub good_text: Color32,
    pub warn: Color32,
    pub warn_text: Color32,
    pub bad: Color32,
    pub bad_text: Color32,
    pub toggle_off: Color32,
    /// Outline of an off switch (≥ 3:1 against the card).
    pub toggle_ring: Color32,
    pub shadow: Color32,
}

const fn hex(v: u32) -> Color32 {
    Color32::from_rgb((v >> 16) as u8, (v >> 8) as u8, v as u8)
}

pub const LIGHT: Palette = Palette {
    canvas: hex(0xFFFFFF),
    surface: hex(0xFFFFFF),
    sunken: hex(0xF3F3F6),
    hover: hex(0xEBEBF0),
    border: hex(0xE7E7ED),
    border_strong: hex(0xD6D6DF),
    text: hex(0x0B0B10),
    muted: hex(0x5A5D68),
    faint: hex(0x707481),
    accent: hex(0x3D3DF5),
    accent_hover: hex(0x2C2CE0),
    accent_soft: hex(0xEDEDFE),
    accent_text: hex(0x3232E4),
    selection: hex(0xCDCDFC),
    good: hex(0x16A34A),
    good_text: hex(0x12804A),
    warn: hex(0xF59E0B),
    warn_text: hex(0xA9520A),
    bad: hex(0xEF4444),
    bad_text: hex(0xD12A2A),
    toggle_off: hex(0xE6E6EC),
    toggle_ring: hex(0x8E8E9C),
    shadow: Color32::from_black_alpha(24),
};

pub const DARK: Palette = Palette {
    canvas: hex(0x0B0B0E),
    surface: hex(0x131317),
    sunken: hex(0x1B1B21),
    hover: hex(0x23232A),
    border: hex(0x25252D),
    border_strong: hex(0x34343F),
    text: hex(0xEDEDF1),
    muted: hex(0xA3A6B1),
    faint: hex(0x888B98),
    accent: hex(0x5C5CFF),
    accent_hover: hex(0x6F6FFF),
    accent_soft: hex(0x1E1E44),
    accent_text: hex(0xA3A3FF),
    selection: hex(0x30307C),
    good: hex(0x2FBF71),
    good_text: hex(0x4ADE80),
    warn: hex(0xF5A524),
    warn_text: hex(0xFBBF24),
    bad: hex(0xF2555A),
    bad_text: hex(0xFF7D81),
    toggle_off: hex(0x2C2C35),
    toggle_ring: hex(0x72727F),
    shadow: Color32::from_black_alpha(120),
};

static DARK_MODE: AtomicBool = AtomicBool::new(false);
static MOTION: AtomicBool = AtomicBool::new(true);

pub fn pal() -> &'static Palette {
    if DARK_MODE.load(Ordering::Relaxed) {
        &DARK
    } else {
        &LIGHT
    }
}

pub fn is_dark() -> bool {
    DARK_MODE.load(Ordering::Relaxed)
}

pub fn canvas() -> Color32 {
    pal().canvas
}
pub fn surface() -> Color32 {
    pal().surface
}
pub fn sunken() -> Color32 {
    pal().sunken
}
pub fn border() -> Color32 {
    pal().border
}
pub fn text() -> Color32 {
    pal().text
}
pub fn muted() -> Color32 {
    pal().muted
}
pub fn faint() -> Color32 {
    pal().faint
}
pub fn accent() -> Color32 {
    pal().accent
}

/// Linear blend in sRGB space (fine for UI tints).
pub fn mix(a: Color32, b: Color32, t: f32) -> Color32 {
    let t = t.clamp(0.0, 1.0);
    let l = |x: u8, y: u8| (x as f32 + (y as f32 - x as f32) * t).round() as u8;
    Color32::from_rgba_unmultiplied(l(a.r(), b.r()), l(a.g(), b.g()), l(a.b(), b.b()), l(a.a(), b.a()))
}

/// A colour as a soft, opaque tint over the card surface.
pub fn soft(c: Color32) -> Color32 {
    mix(surface(), c, if is_dark() { 0.16 } else { 0.10 })
}

/// Semantic colour roles. `fg` is always readable as text on a card; `solid` is for dots, bars and
/// fills; `soft` is the tinted background.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Tone {
    Neutral,
    Accent,
    Good,
    Warn,
    Bad,
}

impl Tone {
    pub fn fg(self) -> Color32 {
        let p = pal();
        match self {
            Tone::Neutral => p.muted,
            Tone::Accent => p.accent_text,
            Tone::Good => p.good_text,
            Tone::Warn => p.warn_text,
            Tone::Bad => p.bad_text,
        }
    }
    pub fn solid(self) -> Color32 {
        let p = pal();
        match self {
            Tone::Neutral => p.faint,
            Tone::Accent => p.accent,
            Tone::Good => p.good,
            Tone::Warn => p.warn,
            Tone::Bad => p.bad,
        }
    }
    pub fn soft(self) -> Color32 {
        match self {
            Tone::Neutral => sunken(),
            Tone::Accent => pal().accent_soft,
            t => soft(t.solid()),
        }
    }
}

/// Text colour for a load percentage: normal until it gets high.
pub fn load_color(load: f32) -> Color32 {
    if load >= 90.0 {
        Tone::Bad.fg()
    } else if load >= 70.0 {
        Tone::Warn.fg()
    } else {
        text()
    }
}

// ================================================================== type

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum W {
    Regular,
    Medium,
    Semi,
    Bold,
}

const FAMILY_MEDIUM: &str = "geist-medium";
const FAMILY_SEMI: &str = "geist-semibold";
const FAMILY_BOLD: &str = "geist-bold";
const FAMILY_ICONS: &str = "lucide";

pub fn family(w: W) -> FontFamily {
    match w {
        W::Regular => FontFamily::Proportional,
        W::Medium => FontFamily::Name(FAMILY_MEDIUM.into()),
        W::Semi => FontFamily::Name(FAMILY_SEMI.into()),
        W::Bold => FontFamily::Name(FAMILY_BOLD.into()),
    }
}

pub fn font(size: f32, w: W) -> FontId {
    FontId::new(size, family(w))
}

pub fn icon_font(size: f32) -> FontId {
    FontId::new(size, FontFamily::Name(FAMILY_ICONS.into()))
}

/// Geist (four static weights, tabular digits) + Lucide icons, both embedded; Windows' own Segoe UI
/// as the fallback for anything else (non-Latin process names...). Lucide also supplies the one
/// symbol egui draws itself (the submenu arrow, U+23F5), so no symbol font has to be loaded.
fn install_fonts(ctx: &Context) {
    let mut fd = FontDefinitions::empty();
    let embedded: [(&str, &'static [u8]); 5] = [
        ("geist", include_bytes!("../../assets/fonts/Geist-Regular.ttf")),
        (FAMILY_MEDIUM, include_bytes!("../../assets/fonts/Geist-Medium.ttf")),
        (FAMILY_SEMI, include_bytes!("../../assets/fonts/Geist-SemiBold.ttf")),
        (FAMILY_BOLD, include_bytes!("../../assets/fonts/Geist-Bold.ttf")),
        (FAMILY_ICONS, include_bytes!("../../assets/fonts/lucide-avix.ttf")),
    ];
    for (name, bytes) in embedded {
        let mut data = FontData::from_static(bytes);
        if name == FAMILY_ICONS {
            data.tweak.y_offset_factor = 0.06;
        }
        fd.font_data.insert(name.into(), Arc::new(data));
    }
    let mut fallbacks = Vec::new();
    let fonts_dir = std::env::var_os("WINDIR").map(std::path::PathBuf::from).unwrap_or_else(|| "C:\\Windows".into()).join("Fonts");
    for (name, file) in [("segoe", "segoeui.ttf")] {
        if let Ok(bytes) = std::fs::read(fonts_dir.join(file)) {
            fd.font_data.insert(name.into(), Arc::new(FontData::from_owned(bytes)));
            fallbacks.push(name.to_string());
        }
    }
    let chain = |first: &str| {
        let mut v = vec![first.to_string()];
        v.extend(fallbacks.iter().cloned());
        v.push(FAMILY_ICONS.to_string());
        v
    };
    fd.families.insert(FontFamily::Proportional, chain("geist"));
    fd.families.insert(FontFamily::Monospace, chain("geist"));
    fd.families.insert(FontFamily::Name(FAMILY_MEDIUM.into()), chain(FAMILY_MEDIUM));
    fd.families.insert(FontFamily::Name(FAMILY_SEMI.into()), chain(FAMILY_SEMI));
    fd.families.insert(FontFamily::Name(FAMILY_BOLD.into()), chain(FAMILY_BOLD));
    fd.families.insert(FontFamily::Name(FAMILY_ICONS.into()), vec![FAMILY_ICONS.into()]);
    ctx.set_fonts(fd);
}

/// Body text, optionally sized/weighted.
pub fn t(text: impl Into<String>) -> RichText {
    RichText::new(text.into()).font(font(13.5, W::Regular)).color(text_color())
}
fn text_color() -> Color32 {
    text()
}
pub fn tw(text: impl Into<String>, size: f32, w: W) -> RichText {
    RichText::new(text.into()).font(font(size, w)).color(text_color())
}
pub fn strong(text: impl Into<String>) -> RichText {
    tw(text, 13.5, W::Medium)
}
pub fn muted_t(text: impl Into<String>) -> RichText {
    RichText::new(text.into()).font(font(13.5, W::Regular)).color(muted())
}
pub fn small(text: impl Into<String>) -> RichText {
    RichText::new(text.into()).font(font(12.0, W::Regular)).color(muted())
}
pub fn label_t(text: impl Into<String>) -> RichText {
    RichText::new(text.into()).font(font(12.0, W::Medium)).color(muted())
}
pub fn time_t(text: impl Into<String>) -> RichText {
    RichText::new(text.into()).font(font(11.5, W::Regular)).color(faint())
}
pub fn icon_t(c: char, size: f32) -> RichText {
    RichText::new(c.to_string()).font(icon_font(size))
}

/// Lucide glyphs embedded in assets/fonts/lucide-avix.ttf (the full subset, used or not).
#[allow(dead_code)]
pub mod icon {
    pub const DASHBOARD: char = '\u{e1c1}';
    pub const ZAP: char = '\u{e1b4}';
    pub const CPU: char = '\u{e0ad}';
    pub const SPARKLES: char = '\u{e417}';
    pub const HEART_PULSE: char = '\u{e372}';
    pub const SETTINGS: char = '\u{e158}';
    pub const SEARCH: char = '\u{e155}';
    pub const SUN: char = '\u{e17c}';
    pub const MOON: char = '\u{e122}';
    pub const MONITOR: char = '\u{e121}';
    pub const PAUSE: char = '\u{e132}';
    pub const PLAY: char = '\u{e140}';
    pub const REFRESH: char = '\u{e149}';
    pub const CHEVRON_DOWN: char = '\u{e071}';
    pub const CHEVRON_RIGHT: char = '\u{e073}';
    pub const CHEVRON_UP: char = '\u{e074}';
    pub const X: char = '\u{e1b2}';
    pub const CHECK: char = '\u{e070}';
    pub const ARROW_RIGHT: char = '\u{e04d}';
    pub const PLUS: char = '\u{e141}';
    pub const TRASH: char = '\u{e18e}';
    pub const ELLIPSIS: char = '\u{e0ba}';
    pub const EXTERNAL: char = '\u{e0bd}';
    pub const DOWNLOAD: char = '\u{e0b6}';
    pub const SHIELD_CHECK: char = '\u{e1ff}';
    pub const ALERT: char = '\u{e193}';
    pub const INFO: char = '\u{e0fe}';
    pub const CIRCLE_CHECK: char = '\u{e226}';
    pub const FOLDER: char = '\u{e247}';
    pub const POWER: char = '\u{e144}';
    pub const UNDO: char = '\u{e14c}';
    pub const ARROW_UP: char = '\u{e04e}';
    pub const ARROW_DOWN: char = '\u{e046}';
    pub const GAMEPAD: char = '\u{e0e2}';
    pub const MEMORY: char = '\u{e44a}';
    pub const CHIP: char = '\u{e61f}';
    pub const GAUGE: char = '\u{e1bf}';
    pub const ACTIVITY: char = '\u{e038}';
    pub const TIMER: char = '\u{e1e0}';
    pub const BELL: char = '\u{e05d}';
    pub const KEYBOARD: char = '\u{e284}';
    pub const GLOBE: char = '\u{e0eb}';
    pub const LOCK: char = '\u{e10f}';
    pub const ROCKET: char = '\u{e286}';
    pub const PACKAGE: char = '\u{e12d}';
    pub const CIRCLE_X: char = '\u{e088}';
    pub const MINUS: char = '\u{e120}';
    pub const LIST: char = '\u{e10c}';
    pub const DRIVE: char = '\u{e0f0}';
    pub const NETWORK: char = '\u{e129}';
    pub const CIRCLE_ALERT: char = '\u{e07b}';
    pub const LAYERS: char = '\u{e104}';
    pub const SUN_MOON: char = '\u{e2b2}';
    pub const WIFI: char = '\u{e1ae}';
    // Added from lucide-static 1.73.0, which numbers icons differently from the set above: these
    // keep that version's codepoints, none of which the older glyphs use.
    pub const STAR: char = '\u{e176}';
    pub const HEART: char = '\u{e0f2}';
    pub const COFFEE: char = '\u{e096}';
    pub const GIT_PULL_REQUEST: char = '\u{e0e5}';
    pub const BUG: char = '\u{e20c}';
    pub const MESSAGE: char = '\u{e116}';
}

pub fn paint_icon(ui: &Ui, center: Pos2, c: char, size: f32, color: Color32) {
    ui.painter().text(center, Align2::CENTER_CENTER, c, icon_font(size), color);
}

/// A standalone icon widget (occupies a square).
pub fn icon_label(ui: &mut Ui, c: char, size: f32, color: Color32) -> Response {
    let (rect, resp) = ui.allocate_exact_size(vec2(size + 2.0, size + 2.0), Sense::hover());
    paint_icon(ui, rect.center(), c, size, color);
    resp
}

// ================================================================== setup & motion

pub fn setup(ctx: &Context, motion: bool) {
    install_fonts(ctx);
    ctx.set_visuals_of(Theme::Light, visuals(&LIGHT, false));
    ctx.set_visuals_of(Theme::Dark, visuals(&DARK, true));
    ctx.all_styles_mut(|s| {
        s.spacing.item_spacing = vec2(8.0, 8.0);
        s.spacing.button_padding = vec2(14.0, 7.0);
        s.spacing.interact_size = vec2(40.0, 32.0);
        s.spacing.combo_width = 200.0;
        s.spacing.slider_width = 200.0;
        s.spacing.menu_margin = Margin::same(6);
        s.spacing.window_margin = Margin::same(18);
        s.spacing.icon_width = 18.0;
        s.spacing.icon_width_inner = 10.0;
        s.spacing.icon_spacing = 8.0;
        s.spacing.indent = 20.0;
        s.spacing.combo_height = 320.0;
        s.spacing.scroll = egui::style::ScrollStyle::floating();
        s.spacing.scroll.floating_width = 6.0;
        s.spacing.scroll.active_handle_opacity = 0.28;
        s.spacing.scroll.interact_handle_opacity = 0.5;
        s.text_styles.insert(TextStyle::Heading, font(22.0, W::Semi));
        s.text_styles.insert(TextStyle::Body, font(13.5, W::Regular));
        s.text_styles.insert(TextStyle::Button, font(13.5, W::Medium));
        s.text_styles.insert(TextStyle::Small, font(12.0, W::Regular));
        s.text_styles.insert(TextStyle::Monospace, FontId::monospace(13.0));
        s.interaction.tooltip_delay = 0.25;
        s.animation_time = if motion { 0.14 } else { 0.0 };
    });
    MOTION.store(motion, Ordering::Relaxed);
}

fn visuals(p: &Palette, dark: bool) -> egui::Visuals {
    let mut v = if dark { egui::Visuals::dark() } else { egui::Visuals::light() };
    v.panel_fill = p.canvas;
    v.window_fill = p.surface;
    v.extreme_bg_color = p.sunken;
    v.faint_bg_color = p.sunken;
    v.code_bg_color = p.sunken;
    v.override_text_color = Some(p.text);
    v.hyperlink_color = p.accent_text;
    v.warn_fg_color = p.warn_text;
    v.error_fg_color = p.bad_text;
    v.window_stroke = Stroke::new(1.0_f32, p.border);
    v.window_corner_radius = CornerRadius::same(16);
    v.menu_corner_radius = CornerRadius::same(12);
    let shadow = Shadow { offset: [0, 10], blur: 30, spread: 0, color: p.shadow };
    v.window_shadow = shadow;
    v.popup_shadow = Shadow { offset: [0, 8], blur: 24, spread: 0, color: p.shadow };
    v.selection.bg_fill = p.selection;
    v.selection.stroke = Stroke::new(1.0_f32, p.accent_text);
    v.text_cursor.stroke = Stroke::new(2.0_f32, p.accent);
    v.slider_trailing_fill = true;
    v.handle_shape = egui::style::HandleShape::Circle;
    v.striped = false;
    v.indent_has_left_vline = false;
    v.collapsing_header_frame = false;
    v.interact_cursor = Some(egui::CursorIcon::PointingHand);

    let r = CornerRadius::same(10);
    let w = &mut v.widgets;
    w.noninteractive.bg_fill = p.surface;
    w.noninteractive.weak_bg_fill = p.surface;
    w.noninteractive.bg_stroke = Stroke::new(1.0_f32, p.border);
    w.noninteractive.fg_stroke = Stroke::new(1.0_f32, p.text);
    w.noninteractive.corner_radius = r;
    for (st, fill, stroke, fg) in [
        (&mut w.inactive, p.sunken, Stroke::NONE, p.text),
        (&mut w.hovered, p.hover, Stroke::new(1.0_f32, p.border_strong), p.text),
        (&mut w.active, p.border_strong, Stroke::new(1.0_f32, p.accent), p.text),
        (&mut w.open, p.hover, Stroke::new(1.0_f32, p.border_strong), p.text),
    ] {
        st.bg_fill = fill;
        st.weak_bg_fill = fill;
        st.bg_stroke = stroke;
        st.fg_stroke = Stroke::new(1.5_f32, fg);
        st.corner_radius = r;
        st.expansion = 0.0;
    }
    v
}

/// Called at the top of every frame: tells the token functions which palette is live and
/// whether motion is on.
pub fn sync(ctx: &Context, motion: bool) {
    DARK_MODE.store(ctx.style().visuals.dark_mode, Ordering::Relaxed);
    if MOTION.swap(motion, Ordering::Relaxed) != motion {
        ctx.all_styles_mut(|s| s.animation_time = if motion { 0.14 } else { 0.0 });
    }
}

pub fn motion() -> bool {
    MOTION.load(Ordering::Relaxed)
}

/// Eased 0..1 for a boolean (snaps when animations are off).
pub fn ease_bool(ctx: &Context, id: Id, on: bool, secs: f32) -> f32 {
    if !motion() {
        return if on { 1.0 } else { 0.0 };
    }
    ctx.animate_bool_with_time_and_easing(id, on, secs, easing::cubic_out)
}

#[derive(Clone, Copy)]
struct Tween {
    from: f32,
    to: f32,
    t0: f64,
}

/// Eases towards `target` whenever it changes (cubic-out). Snaps when animations are off.
pub fn ease_value(ctx: &Context, id: Id, target: f32, secs: f32) -> f32 {
    let now = ctx.input(|i| i.time);
    if !motion() || !target.is_finite() {
        ctx.data_mut(|d| d.insert_temp(id, Tween { from: target, to: target, t0: now }));
        return target;
    }
    let at = |tw: Tween| {
        let k = (((now - tw.t0) as f32) / secs).clamp(0.0, 1.0);
        (tw.from + (tw.to - tw.from) * easing::cubic_out(k), k)
    };
    let mut tw = ctx.data_mut(|d| *d.get_temp_mut_or_insert_with(id, || Tween { from: target, to: target, t0: now - 10.0 }));
    if (tw.to - target).abs() > 1e-4 {
        let (cur, _) = at(tw);
        tw = Tween { from: cur, to: target, t0: now };
        ctx.data_mut(|d| d.insert_temp(id, tw));
    }
    let (v, k) = at(tw);
    if k < 1.0 {
        ctx.request_repaint();
    }
    v
}

/// 0..1 progress of an entrance that started at `since` (seconds, egui time).
pub fn entrance(ctx: &Context, since: f64, secs: f32) -> f32 {
    if !motion() {
        return 1.0;
    }
    let k = (((ctx.input(|i| i.time) - since) as f32) / secs).clamp(0.0, 1.0);
    if k < 1.0 {
        ctx.request_repaint();
    }
    easing::cubic_out(k)
}

// ================================================================== containers

pub fn card_frame() -> Frame {
    Frame::new()
        .fill(surface())
        .stroke(Stroke::new(1.0_f32, border()))
        .corner_radius(CornerRadius::same(16))
        .inner_margin(Margin::same(18))
}

pub fn card<R>(ui: &mut Ui, add: impl FnOnce(&mut Ui) -> R) -> R {
    card_frame()
        .show(ui, |ui| {
            ui.set_width(ui.available_width());
            add(ui)
        })
        .inner
}

pub fn card_tinted<R>(ui: &mut Ui, tone: Tone, add: impl FnOnce(&mut Ui) -> R) -> R {
    Frame::new()
        .fill(tone.soft())
        .stroke(Stroke::new(1.0_f32, mix(tone.soft(), tone.solid(), 0.22)))
        .corner_radius(CornerRadius::same(16))
        .inner_margin(Margin::same(18))
        .show(ui, |ui| {
            ui.set_width(ui.available_width());
            add(ui)
        })
        .inner
}

/// Floating surface (toasts, save bar, menus drawn by hand).
pub fn float_frame() -> Frame {
    Frame::new()
        .fill(surface())
        .stroke(Stroke::new(1.0_f32, pal().border_strong))
        .corner_radius(CornerRadius::same(14))
        .inner_margin(Margin::symmetric(16, 12))
        .shadow(Shadow { offset: [0, 10], blur: 30, spread: 0, color: pal().shadow })
}

pub fn page_header(ui: &mut Ui, title: &str, subtitle: &str) {
    ui.label(tw(title, 26.0, W::Semi).extra_letter_spacing(-0.5));
    if !subtitle.is_empty() {
        ui.add_space(-2.0);
        ui.label(muted_t(subtitle));
    }
    ui.add_space(14.0);
}

/// Card title.
pub fn section(ui: &mut Ui, title: &str) {
    ui.label(tw(title, 15.0, W::Semi).extra_letter_spacing(-0.15));
}

/// Card title with something right-aligned on the same line.
pub fn section_with(ui: &mut Ui, title: &str, right: impl FnOnce(&mut Ui)) {
    ui.horizontal(|ui| {
        section(ui, title);
        ui.with_layout(Layout::right_to_left(Align::Center), right);
    });
}

pub fn divider(ui: &mut Ui) {
    let (rect, _) = ui.allocate_exact_size(vec2(ui.available_width(), 1.0), Sense::hover());
    ui.painter().hline(rect.x_range(), rect.center().y, Stroke::new(1.0_f32, border()));
}

/// Two-column row: text on the left (wraps), a control on the right, vertically centred on each
/// other. Collapses to a stack when the row is too narrow.
pub fn split_row<R>(
    ui: &mut Ui,
    id_salt: impl Hash,
    right_w: f32,
    left: impl FnOnce(&mut Ui),
    right: impl FnOnce(&mut Ui) -> R,
) -> R {
    let w = ui.available_width();
    if w < right_w + 220.0 {
        left(ui);
        ui.add_space(2.0);
        return ui.allocate_ui_with_layout(vec2(w, 0.0), Layout::left_to_right(Align::Center), right).inner;
    }
    let id = ui.id().with(("split-row", id_salt));
    let (hl_prev, hr_prev): (f32, f32) = ui.data(|d| d.get_temp(id)).unwrap_or((0.0, 0.0));
    let h = hl_prev.max(hr_prev);
    let top = ui.cursor().min;
    let gap = 20.0;
    let left_w = w - right_w - gap;
    let mut lui = ui.new_child(
        UiBuilder::new()
            .max_rect(Rect::from_min_size(pos2(top.x, top.y + (h - hl_prev).max(0.0) / 2.0), vec2(left_w, 10_000.0)))
            .layout(Layout::top_down(Align::Min)),
    );
    lui.spacing_mut().item_spacing.y = 2.0;
    left(&mut lui);
    let lrect = lui.min_rect();
    let mut rui = ui.new_child(
        UiBuilder::new()
            .max_rect(Rect::from_min_size(pos2(top.x + w - right_w, top.y + (h - hr_prev).max(0.0) / 2.0), vec2(right_w, 10_000.0)))
            .layout(Layout::right_to_left(Align::Min)),
    );
    let r = right(&mut rui);
    let rrect = rui.min_rect();
    let (hl, hr) = (lrect.height(), rrect.height());
    if (hl - hl_prev).abs() > 0.5 || (hr - hr_prev).abs() > 0.5 {
        ui.data_mut(|d| d.insert_temp(id, (hl, hr)));
        ui.ctx().request_repaint();
    }
    ui.advance_cursor_after_rect(Rect::from_min_size(top, vec2(w, hl.max(hr))));
    r
}

/// Label + hint, the left side of a settings row.
pub fn row_label(ui: &mut Ui, label: &str, hint: &str) {
    ui.add(egui::Label::new(strong(label)).wrap());
    if !hint.is_empty() {
        ui.add(egui::Label::new(small(hint)).wrap());
    }
}

/// Settings row with a switch on the right. Returns true when it changed.
pub fn toggle_row(ui: &mut Ui, on: &mut bool, label: &str, hint: &str) -> bool {
    let changed = split_row(ui, label, 44.0, |ui| row_label(ui, label, hint), |ui| toggle(ui, on).changed());
    ui.add_space(6.0);
    changed
}

// ================================================================== controls

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    Primary,
    Secondary,
    Outline,
    Ghost,
    Danger,
}

pub struct Btn<'a> {
    text: &'a str,
    icon: Option<char>,
    kind: Kind,
    small: bool,
    enabled: bool,
    min_width: f32,
}

impl<'a> Btn<'a> {
    pub fn new(kind: Kind, text: &'a str) -> Self {
        Self { text, icon: None, kind, small: false, enabled: true, min_width: 0.0 }
    }
    pub fn icon(mut self, c: char) -> Self {
        self.icon = Some(c);
        self
    }
    pub fn small(mut self) -> Self {
        self.small = true;
        self
    }
    pub fn enabled(mut self, on: bool) -> Self {
        self.enabled = on;
        self
    }
    pub fn min_width(mut self, w: f32) -> Self {
        self.min_width = w;
        self
    }

    pub fn show(self, ui: &mut Ui) -> Response {
        let enabled = self.enabled && ui.is_enabled();
        let h = if self.small { 28.0 } else { 34.0 };
        let pad = if self.small { 11.0 } else { 15.0 };
        let isz = if self.small { 14.0 } else { 15.0 };
        let gap = 7.0;
        let galley = (!self.text.is_empty()).then(|| {
            ui.painter().layout_no_wrap(self.text.to_owned(), font(if self.small { 12.5 } else { 13.5 }, W::Medium), Color32::PLACEHOLDER)
        });
        let tw = galley.as_ref().map(|g| g.size().x).unwrap_or(0.0);
        let content = tw + if self.icon.is_some() { isz + if galley.is_some() { gap } else { 0.0 } } else { 0.0 };
        let w = if galley.is_none() { h } else { (content + pad * 2.0).max(self.min_width) };
        let sense = if enabled { Sense::click() } else { Sense::hover() };
        let (rect, resp) = ui.allocate_exact_size(vec2(w, h), sense);
        if ui.is_rect_visible(rect) {
            let p = pal();
            let hov = ease_bool(ui.ctx(), resp.id.with("hover"), enabled && resp.hovered(), 0.12);
            let (base, hot, fg, stroke) = match self.kind {
                Kind::Primary => (p.accent, p.accent_hover, Color32::WHITE, Stroke::NONE),
                Kind::Secondary => (p.sunken, p.hover, p.text, Stroke::NONE),
                Kind::Outline => (p.surface, p.sunken, p.text, Stroke::new(1.0_f32, p.border_strong)),
                Kind::Ghost => (Color32::TRANSPARENT, p.hover, if hov > 0.5 { p.text } else { p.muted }, Stroke::NONE),
                Kind::Danger => (Tone::Bad.soft(), mix(Tone::Bad.soft(), p.bad, 0.16), p.bad_text, Stroke::NONE),
            };
            let mut fill = mix(base, hot, hov);
            if enabled && resp.is_pointer_button_down_on() {
                fill = mix(fill, p.text, 0.06);
            }
            let a = if enabled { 1.0 } else { 0.45 };
            let cr = CornerRadius::same((h / 2.0) as u8);
            ui.painter().rect(rect, cr, fill.gamma_multiply(a), stroke, StrokeKind::Inside);
            let fg = fg.gamma_multiply(a);
            let mut x = rect.center().x - content / 2.0;
            if let Some(c) = self.icon {
                paint_icon(ui, pos2(x + isz / 2.0, rect.center().y), c, isz, fg);
                x += isz + gap;
            }
            if let Some(g) = galley {
                ui.painter().galley(pos2(x, rect.center().y - g.size().y / 2.0), g, fg);
            }
            if resp.has_focus() {
                ui.painter().rect_stroke(rect.expand(2.0), CornerRadius::same((h / 2.0 + 2.0) as u8), Stroke::new(2.0_f32, p.accent.gamma_multiply(0.6)), StrokeKind::Outside);
            }
        }
        if enabled {
            resp.on_hover_cursor(egui::CursorIcon::PointingHand)
        } else {
            resp
        }
    }
}

pub fn primary(ui: &mut Ui, text: &str) -> Response {
    Btn::new(Kind::Primary, text).show(ui)
}
pub fn secondary(ui: &mut Ui, text: &str) -> Response {
    Btn::new(Kind::Secondary, text).show(ui)
}
pub fn ghost(ui: &mut Ui, text: &str) -> Response {
    Btn::new(Kind::Ghost, text).show(ui)
}


/// Pill segmented control with a sliding thumb. Returns the clicked index.
/// Returns the clicked index (if any) and the control's rect.
pub fn segmented(ui: &mut Ui, id_salt: impl Hash, items: &[(Option<char>, &str)], selected: usize, small: bool) -> (Option<usize>, Rect) {
    let id = ui.id().with(("segmented", id_salt));
    let h = if small { 30.0 } else { 34.0 };
    let pad = if small { 11.0 } else { 14.0 };
    let f = font(if small { 12.5 } else { 13.0 }, W::Medium);
    let isz = 14.0;
    let widths: Vec<f32> = items
        .iter()
        .map(|(ic, label)| {
            let tw = ui.painter().layout_no_wrap((*label).to_owned(), f.clone(), Color32::PLACEHOLDER).size().x;
            let iw = if ic.is_some() { isz + if label.is_empty() { 0.0 } else { 6.0 } } else { 0.0 };
            tw + iw + pad * 2.0
        })
        .collect();
    let total: f32 = widths.iter().sum::<f32>() + 6.0;
    let (rect, _) = ui.allocate_exact_size(vec2(total, h), Sense::hover());
    let p = pal();
    ui.painter().rect_filled(rect, CornerRadius::same((h / 2.0) as u8), p.sunken);
    let mut clicked = None;
    let mut x = rect.left() + 3.0;
    let mut sel_x = x;
    let mut cells = Vec::new();
    for (i, w) in widths.iter().enumerate() {
        let cell = Rect::from_min_size(pos2(x, rect.top() + 3.0), vec2(*w, h - 6.0));
        if i == selected {
            sel_x = x;
        }
        cells.push(cell);
        x += w;
    }
    let thumb_x = ease_value(ui.ctx(), id.with("x"), sel_x - rect.left(), 0.22) + rect.left();
    let thumb_w = ease_value(ui.ctx(), id.with("w"), widths.get(selected).copied().unwrap_or(0.0), 0.22);
    if selected < items.len() {
        let thumb = Rect::from_min_size(pos2(thumb_x, rect.top() + 3.0), vec2(thumb_w, h - 6.0));
        let fill = if is_dark() { p.border_strong } else { p.surface };
        ui.painter().rect_filled(thumb, CornerRadius::same(((h - 6.0) / 2.0) as u8), fill);
        if !is_dark() {
            ui.painter().rect_stroke(thumb, CornerRadius::same(((h - 6.0) / 2.0) as u8), Stroke::new(1.0_f32, p.border), StrokeKind::Inside);
        }
    }
    for (i, ((ic, label), cell)) in items.iter().zip(cells).enumerate() {
        let resp = ui.interact(cell, id.with(i), Sense::click()).on_hover_cursor(egui::CursorIcon::PointingHand);
        if resp.clicked() && i != selected {
            clicked = Some(i);
        }
        let color = if i == selected || resp.hovered() { p.text } else { p.muted };
        let tw = ui.painter().layout_no_wrap((*label).to_owned(), f.clone(), color);
        let iw = if ic.is_some() { isz + if label.is_empty() { 0.0 } else { 6.0 } } else { 0.0 };
        let mut cx = cell.center().x - (tw.size().x + iw) / 2.0;
        if let Some(c) = ic {
            paint_icon(ui, pos2(cx + isz / 2.0, cell.center().y), *c, isz, color);
            cx += iw;
        }
        ui.painter().galley(pos2(cx, cell.center().y - tw.size().y / 2.0), tw, color);
    }
    (clicked, rect)
}

/// Filter pill (used in a `horizontal_wrapped` row).
pub fn pill_tab(ui: &mut Ui, label: &str, selected: bool, count: Option<usize>) -> Response {
    let f = font(13.0, W::Medium);
    let text = match count {
        Some(n) => format!("{label}  {n}"),
        None => label.to_string(),
    };
    let g = ui.painter().layout_no_wrap(text, f, Color32::PLACEHOLDER);
    let size = vec2(g.size().x + 26.0, 32.0);
    let (rect, resp) = ui.allocate_exact_size(size, Sense::click());
    let p = pal();
    let hov = ease_bool(ui.ctx(), resp.id.with("hover"), resp.hovered(), 0.12);
    let sel = ease_bool(ui.ctx(), resp.id.with("sel"), selected, 0.18);
    let fill = mix(mix(Color32::TRANSPARENT, p.hover, hov * (1.0 - sel)), p.accent, sel);
    ui.painter().rect_filled(rect, CornerRadius::same(16), fill);
    if sel < 0.5 && hov < 0.01 {
        ui.painter().rect_stroke(rect, CornerRadius::same(16), Stroke::new(1.0_f32, p.border), StrokeKind::Inside);
    }
    let fg = mix(if hov > 0.5 { p.text } else { p.muted }, Color32::WHITE, sel);
    ui.painter().galley(pos2(rect.left() + 13.0, rect.center().y - g.size().y / 2.0), g, fg);
    resp.on_hover_cursor(egui::CursorIcon::PointingHand)
}

/// Switch.
pub fn toggle(ui: &mut Ui, on: &mut bool) -> Response {
    let size = vec2(38.0, 22.0);
    let (rect, mut resp) = ui.allocate_exact_size(size, Sense::click());
    if resp.clicked() {
        *on = !*on;
        resp.mark_changed();
    }
    if ui.is_rect_visible(rect) {
        let p = pal();
        let k = ease_bool(ui.ctx(), resp.id, *on, 0.18);
        let mut track = mix(p.toggle_off, p.accent, k);
        if resp.hovered() {
            track = mix(track, p.text, 0.06);
        }
        ui.painter().rect_filled(rect, CornerRadius::same(11), track);
        if k < 1.0 {
            ui.painter().rect_stroke(rect, CornerRadius::same(11), Stroke::new(1.5_f32, p.toggle_ring.gamma_multiply(1.0 - k)), StrokeKind::Inside);
        }
        let x = egui::lerp((rect.left() + 11.0)..=(rect.right() - 11.0), k);
        let c = pos2(x, rect.center().y);
        ui.painter().circle_filled(c + vec2(0.0, 1.0), 8.5, Color32::from_black_alpha(28));
        ui.painter().circle_filled(c, 8.0, Color32::WHITE);
        if resp.has_focus() {
            ui.painter().rect_stroke(rect.expand(2.0), CornerRadius::same(13), Stroke::new(2.0_f32, p.accent.gamma_multiply(0.6)), StrokeKind::Outside);
        }
    }
    resp.on_hover_cursor(egui::CursorIcon::PointingHand)
}

/// Checkbox with an accent fill when checked.
pub fn checkbox(ui: &mut Ui, on: &mut bool, label: impl Into<RichText>) -> Response {
    let label: RichText = label.into();
    let g = egui::WidgetText::from(label).into_galley(ui, Some(egui::TextWrapMode::Extend), f32::INFINITY, TextStyle::Body);
    let size = vec2(18.0 + 10.0 + g.size().x, g.size().y.max(20.0));
    let (rect, mut resp) = ui.allocate_exact_size(size, Sense::click());
    if resp.clicked() {
        *on = !*on;
        resp.mark_changed();
    }
    if ui.is_rect_visible(rect) {
        let p = pal();
        let k = ease_bool(ui.ctx(), resp.id, *on, 0.14);
        let b = Rect::from_center_size(pos2(rect.left() + 9.0, rect.center().y), vec2(18.0, 18.0));
        let fill = mix(p.surface, p.accent, k);
        let stroke = if k > 0.5 { Stroke::NONE } else { Stroke::new(1.5_f32, if resp.hovered() { p.faint } else { p.border_strong }) };
        ui.painter().rect(b, CornerRadius::same(5), fill, stroke, StrokeKind::Inside);
        if k > 0.01 {
            paint_icon(ui, b.center(), icon::CHECK, 13.0, Color32::WHITE.gamma_multiply(k));
        }
        ui.painter().galley(pos2(b.right() + 10.0, rect.center().y - g.size().y / 2.0), g, p.text);
    }
    resp.on_hover_cursor(egui::CursorIcon::PointingHand)
}

/// Single-line input in a soft rounded field. `leading` adds an icon inside the field.
pub fn text_field(ui: &mut Ui, id_salt: impl Hash, buf: &mut String, hint: &str, width: f32, leading: Option<char>) -> Response {
    let id = ui.id().with(("field", id_salt));
    let focused = ui.memory(|m| m.has_focus(id));
    let p = pal();
    let lead = if leading.is_some() { 26.0 } else { 0.0 };
    let frame = Frame::new()
        .fill(if focused { p.surface } else { p.sunken })
        .stroke(Stroke::new(1.0_f32, if focused { p.accent } else { Color32::TRANSPARENT }))
        .corner_radius(CornerRadius::same(10))
        .inner_margin(Margin { left: 12 + lead as i8, right: 12, top: 8, bottom: 8 });
    let inner = frame.show(ui, |ui| {
        ui.add(
            egui::TextEdit::singleline(buf)
                .id(id)
                .frame(false)
                .hint_text(RichText::new(hint).color(p.faint))
                .font(font(13.5, W::Regular))
                .desired_width((width - 24.0 - lead).max(40.0))
                .margin(Margin::ZERO),
        )
    });
    if let Some(c) = leading {
        let r = inner.response.rect;
        paint_icon(ui, pos2(r.left() + 12.0 + 8.0, r.center().y), c, 15.0, if focused { p.text } else { p.faint });
    }
    inner.inner
}

/// Small coloured pill (state, risk, counts). Painted as one widget so it wraps.
pub fn chip(ui: &mut Ui, text: impl Into<String>, tone: Tone) -> Response {
    chip_icon(ui, None, text, tone)
}

pub fn chip_icon(ui: &mut Ui, ic: Option<char>, text: impl Into<String>, tone: Tone) -> Response {
    let g = ui.painter().layout_no_wrap(text.into(), font(11.5, W::Medium), tone.fg());
    let iw = if ic.is_some() { 15.0 } else { 0.0 };
    let size = vec2(g.size().x + 18.0 + iw, 22.0);
    let (rect, resp) = ui.allocate_exact_size(size, Sense::hover());
    if ui.is_rect_visible(rect) {
        ui.painter().rect_filled(rect, CornerRadius::same(11), tone.soft());
        if let Some(c) = ic {
            paint_icon(ui, pos2(rect.left() + 9.0 + 5.5, rect.center().y), c, 12.0, tone.fg());
        }
        ui.painter().galley(pos2(rect.left() + 9.0 + iw, rect.center().y - g.size().y / 2.0), g, tone.fg());
    }
    resp
}

/// Removable list item (painted, wraps). Returns true when its × is clicked.
pub fn removable_chip(ui: &mut Ui, text: &str) -> bool {
    let p = pal();
    let g = ui.painter().layout_no_wrap(text.to_string(), font(12.5, W::Regular), p.text);
    let x_w = 22.0;
    let size = vec2(g.size().x + 12.0 + x_w, 28.0);
    let (rect, resp) = ui.allocate_exact_size(size, Sense::click());
    let x_rect = Rect::from_min_size(pos2(rect.right() - x_w - 3.0, rect.top()), vec2(x_w, rect.height()));
    let over_x = resp.hover_pos().is_some_and(|q| x_rect.contains(q));
    if ui.is_rect_visible(rect) {
        ui.painter().rect_filled(rect, CornerRadius::same(14), if over_x { Tone::Bad.soft() } else { p.sunken });
        ui.painter().galley(pos2(rect.left() + 12.0, rect.center().y - g.size().y / 2.0), g, p.text);
        paint_icon(ui, x_rect.center(), icon::X, 13.0, if over_x { p.bad_text } else { p.faint });
    }
    let resp = if over_x { resp.on_hover_text("Remove").on_hover_cursor(egui::CursorIcon::PointingHand) } else { resp };
    resp.clicked() && resp.interact_pointer_pos().is_some_and(|q| x_rect.contains(q))
}

/// Left-aligned, truncated label in a fixed-width cell.
pub fn cell(ui: &mut Ui, width: f32, height: f32, text: impl Into<egui::WidgetText>) -> Response {
    ui.allocate_ui_with_layout(vec2(width, height), Layout::left_to_right(Align::Center), |ui| {
        ui.set_min_size(vec2(width, height));
        ui.add(egui::Label::new(text).truncate())
    })
    .inner
}

/// Right-aligned label in a fixed-width cell (numbers).
pub fn cell_right(ui: &mut Ui, width: f32, height: f32, text: impl Into<egui::WidgetText>) -> Response {
    ui.allocate_ui_with_layout(vec2(width, height), Layout::right_to_left(Align::Center), |ui| {
        ui.set_min_size(vec2(width, height));
        ui.add(egui::Label::new(text).truncate())
    })
    .inner
}

pub fn status_dot(ui: &mut Ui, color: Color32, radius: f32) -> Response {
    let (rect, resp) = ui.allocate_exact_size(vec2(radius * 2.0 + 6.0, radius * 2.0 + 6.0), Sense::hover());
    ui.painter().circle_filled(rect.center(), radius + 3.0, color.gamma_multiply(0.22));
    ui.painter().circle_filled(rect.center(), radius, color);
    resp
}

/// Icon in a soft circle (finding severity, list leading icons).
pub fn icon_badge(ui: &mut Ui, c: char, tone: Tone, size: f32) -> Response {
    let (rect, resp) = ui.allocate_exact_size(vec2(size, size), Sense::hover());
    ui.painter().circle_filled(rect.center(), size / 2.0, tone.soft());
    paint_icon(ui, rect.center(), c, size * 0.5, tone.fg());
    resp
}

/// Smooth area chart (newest sample on the right) with a hover read-out. `fmt` formats a value.
pub fn area_chart(ui: &mut Ui, id_salt: impl Hash, values: &[f32], max: f32, color: Color32, height: f32, fmt: &dyn Fn(f32) -> String) {
    let _ = id_salt;
    let p = pal();
    let label_w = 40.0;
    let (outer, resp) = ui.allocate_exact_size(vec2(ui.available_width(), height + 30.0), Sense::hover());
    let plot = Rect::from_min_max(outer.min + vec2(0.0, 10.0), pos2(outer.right() - label_w, outer.bottom() - 20.0));
    let painter = ui.painter_at(outer);
    let top = values.iter().cloned().fold(max, f32::max).max(1.0);

    // grid
    for k in 0..=2 {
        let v = top * k as f32 / 2.0;
        let y = plot.bottom() - plot.height() * k as f32 / 2.0;
        painter.hline(plot.x_range(), y, Stroke::new(1.0_f32, if k == 0 { p.border_strong } else { p.border }));
        painter.text(pos2(outer.right(), y), Align2::RIGHT_CENTER, fmt(v), font(11.0, W::Regular), p.muted);
    }
    painter.text(pos2(plot.right(), outer.bottom()), Align2::RIGHT_BOTTOM, "now", font(11.0, W::Regular), p.muted);
    // A freshly opened dashboard has only a few samples: wait for ten, then stretch whatever has
    // been collected across the full width (the label says how far back it reaches).
    const MIN_SAMPLES: usize = 10;
    if values.len() < MIN_SAMPLES {
        painter.text(plot.center(), Align2::CENTER_CENTER, format!("Collecting samples… {}/{MIN_SAMPLES}", values.len()), font(12.0, W::Regular), p.muted);
        return;
    }
    let n = values.len();
    painter.text(pos2(plot.left(), outer.bottom()), Align2::LEFT_BOTTOM, format!("{}s ago", n - 1), font(11.0, W::Regular), p.muted);
    let step = plot.width() / (n - 1) as f32;
    let start = 0usize;
    let pts: Vec<Pos2> = values
        .iter()
        .enumerate()
        .map(|(i, v)| pos2(plot.left() + step * i as f32, plot.bottom() - plot.height() * (v / top).clamp(0.0, 1.0)))
        .collect();

    // Gradient fill. The alpha is a linear function of height only (strongest at the highest
    // point, clear at the baseline), so triangle interpolation reproduces it exactly: no seams.
    let mut mesh = Mesh::default();
    let peak = pts.iter().map(|q| q.y).fold(plot.bottom(), f32::min);
    let span = (plot.bottom() - peak).max(1.0);
    // The fade stops short of clear: a pale tint that runs all the way to zero shows 8-bit steps
    // and a white strip above the baseline. Still linear in y, so still seamless.
    let max_a = if is_dark() { 0.36 } else { 0.26 };
    let floor = 0.22;
    let shade = |y: f32| color.gamma_multiply(max_a * (floor + (1.0 - floor) * ((plot.bottom() - y) / span).clamp(0.0, 1.0)));
    let bot_c = shade(plot.bottom());
    for w in pts.windows(2) {
        let base = mesh.vertices.len() as u32;
        mesh.colored_vertex(w[0], shade(w[0].y));
        mesh.colored_vertex(w[1], shade(w[1].y));
        mesh.colored_vertex(pos2(w[1].x, plot.bottom()), bot_c);
        mesh.colored_vertex(pos2(w[0].x, plot.bottom()), bot_c);
        mesh.add_triangle(base, base + 1, base + 2);
        mesh.add_triangle(base, base + 2, base + 3);
    }
    painter.add(egui::Shape::mesh(mesh));
    painter.add(egui::Shape::line(pts.clone(), Stroke::new(2.0_f32, color)));
    let last = *pts.last().unwrap();
    painter.circle_filled(last, 4.5, surface());
    painter.circle_stroke(last, 4.5, Stroke::new(2.0_f32, color));

    // hover read-out
    if let Some(hp) = resp.hover_pos() {
        if plot.x_range().contains(hp.x) {
            let i = (((hp.x - plot.left()) / step).round() as isize - start as isize).clamp(0, values.len() as isize - 1) as usize;
            let pt = pts[i];
            painter.vline(pt.x, plot.y_range(), Stroke::new(1.0_f32, p.border_strong));
            painter.circle_filled(pt, 4.5, color);
            let ago = values.len() - 1 - i;
            let label = if ago == 0 { format!("{}  ·  now", fmt(values[i])) } else { format!("{}  ·  {ago}s ago", fmt(values[i])) };
            let g = painter.layout_no_wrap(label, font(12.0, W::Medium), Color32::WHITE);
            let mut tip = Rect::from_min_size(pos2(pt.x - g.size().x / 2.0 - 10.0, (pt.y - 34.0).max(plot.top() - 6.0)), g.size() + vec2(20.0, 10.0));
            tip = tip.translate(vec2((plot.left() - tip.left()).max(0.0) + (plot.right() - tip.right()).min(0.0), 0.0));
            painter.rect_filled(tip, CornerRadius::same(8), if is_dark() { p.border_strong } else { p.text });
            painter.galley(tip.min + vec2(10.0, 5.0), g, Color32::WHITE);
        }
    }
}

/// Combo box with a Lucide chevron instead of egui's filled triangle.
pub fn combo(id_salt: impl Hash) -> egui::ComboBox {
    egui::ComboBox::from_id_salt(id_salt).icon(|ui, rect, visuals, open, _| {
        paint_icon(ui, rect.center(), if open { icon::CHEVRON_UP } else { icon::CHEVRON_DOWN }, 14.0, visuals.fg_stroke.color);
    })
}

/// Slider: thin rail, accent fill, a filled accent knob with a white rim, and the value as text on
/// the right. Drag or click to set; arrow keys step when focused. Returns true when it changed.
pub fn slider(ui: &mut Ui, id_salt: impl Hash, value: &mut f64, range: RangeInclusive<f64>, step: f64, fmt: &dyn Fn(f64) -> String) -> bool {
    let (lo, hi) = (*range.start(), *range.end());
    let rail_w = 170.0;
    let text_w = 78.0;
    let (rect, resp) = ui.allocate_exact_size(vec2(rail_w + text_w, 26.0), Sense::click_and_drag());
    let _ = id_salt;
    // value on the left, right next to the rail; the rail ends flush with the row's right edge
    let rail = Rect::from_min_size(pos2(rect.left() + text_w + 9.0, rect.center().y - 2.0), vec2(rail_w - 18.0, 4.0));
    let snap = |v: f64| (lo + ((v - lo) / step).round() * step).clamp(lo, hi);
    let mut changed = false;
    if let Some(pos) = resp.interact_pointer_pos() {
        if resp.dragged() || resp.clicked() {
            let f = ((pos.x - rail.left()) / rail.width()).clamp(0.0, 1.0) as f64;
            let v = snap(lo + f * (hi - lo));
            if (v - *value).abs() > f64::EPSILON {
                *value = v;
                changed = true;
            }
        }
    }
    if resp.has_focus() {
        let (l, r) = ui.input(|i| (i.key_pressed(egui::Key::ArrowLeft), i.key_pressed(egui::Key::ArrowRight)));
        if l || r {
            let v = snap(*value + if r { step } else { -step });
            if (v - *value).abs() > f64::EPSILON {
                *value = v;
                changed = true;
            }
        }
    }
    if ui.is_rect_visible(rect) {
        let p = pal();
        let f = (((*value - lo) / (hi - lo)) as f32).clamp(0.0, 1.0);
        let kx = rail.left() + rail.width() * f;
        ui.painter().rect_filled(rail, CornerRadius::same(2), if is_dark() { p.border_strong } else { p.hover });
        ui.painter().rect_filled(Rect::from_min_max(rail.min, pos2(kx, rail.max.y)), CornerRadius::same(2), p.accent);
        let c = pos2(kx, rail.center().y);
        let hot = resp.hovered() || resp.dragged();
        ui.painter().circle_filled(c + vec2(0.0, 1.0), 9.0, Color32::from_black_alpha(30));
        ui.painter().circle_filled(c, 8.5, Color32::WHITE);
        ui.painter().circle_filled(c, if hot { 6.5 } else { 6.0 }, if hot { p.accent_hover } else { p.accent });
        if resp.has_focus() {
            ui.painter().circle_stroke(c, 11.0, Stroke::new(2.0_f32, p.accent.gamma_multiply(0.5)));
        }
        ui.painter().text(pos2(rail.left() - 14.0, rect.center().y), Align2::RIGHT_CENTER, fmt(*value), font(13.0, W::Medium), p.text);
    }
    resp.on_hover_cursor(egui::CursorIcon::PointingHand);
    changed
}

/// Empty / loading placeholder inside a card.
pub fn empty_state(ui: &mut Ui, c: char, title: &str, body: &str) {
    ui.vertical_centered(|ui| {
        ui.add_space(10.0);
        icon_badge(ui, c, Tone::Neutral, 40.0);
        ui.add_space(4.0);
        ui.label(strong(title));
        if !body.is_empty() {
            ui.label(small(body));
        }
        ui.add_space(10.0);
    });
}

/// Spinner + muted text on one line.
pub fn busy(ui: &mut Ui, text: &str) {
    ui.horizontal(|ui| {
        ui.add(egui::Spinner::new().size(16.0).color(accent()));
        ui.label(muted_t(text));
    });
}
