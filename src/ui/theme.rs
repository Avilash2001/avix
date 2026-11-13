//! Avix look & feel + small reusable widgets.

use eframe::egui::{
    self, pos2, vec2, Color32, CornerRadius, FontId, Margin, Response, RichText, Sense, Stroke,
    StrokeKind, TextStyle, Ui,
};

pub const BG: Color32 = Color32::from_rgb(11, 13, 18);
pub const SIDEBAR: Color32 = Color32::from_rgb(14, 16, 22);
pub const CARD: Color32 = Color32::from_rgb(20, 23, 31);
pub const CARD_HI: Color32 = Color32::from_rgb(27, 31, 41);
pub const BORDER: Color32 = Color32::from_rgb(36, 41, 54);
pub const TEXT: Color32 = Color32::from_rgb(232, 235, 242);
pub const MUTED: Color32 = Color32::from_rgb(138, 146, 164);
pub const FAINT: Color32 = Color32::from_rgb(90, 97, 114);
pub const ACCENT: Color32 = Color32::from_rgb(124, 92, 255);
pub const GOOD: Color32 = Color32::from_rgb(52, 211, 153);
pub const WARN: Color32 = Color32::from_rgb(251, 191, 36);
pub const BAD: Color32 = Color32::from_rgb(248, 113, 113);
pub const INFO: Color32 = Color32::from_rgb(96, 165, 250);

pub fn tint(c: Color32, alpha: u8) -> Color32 {
    Color32::from_rgba_unmultiplied(c.r(), c.g(), c.b(), alpha)
}

pub fn apply(ctx: &egui::Context) {
    let mut v = egui::Visuals::dark();
    v.panel_fill = BG;
    v.window_fill = CARD;
    v.extreme_bg_color = Color32::from_rgb(9, 10, 14);
    v.faint_bg_color = CARD_HI;
    v.override_text_color = Some(TEXT);
    v.window_stroke = Stroke::new(1.0_f32, BORDER);
    v.window_corner_radius = CornerRadius::same(12);
    v.menu_corner_radius = CornerRadius::same(10);
    v.selection.bg_fill = tint(ACCENT, 140);
    v.selection.stroke = Stroke::new(1.0_f32, Color32::WHITE);
    v.hyperlink_color = ACCENT;

    let w = &mut v.widgets;
    w.noninteractive.bg_fill = CARD;
    w.noninteractive.weak_bg_fill = CARD;
    w.noninteractive.bg_stroke = Stroke::new(1.0_f32, BORDER);
    w.noninteractive.fg_stroke = Stroke::new(1.0_f32, TEXT);
    w.noninteractive.corner_radius = CornerRadius::same(8);
    for (st, fill, stroke) in [
        (&mut w.inactive, CARD_HI, BORDER),
        (&mut w.hovered, Color32::from_rgb(36, 41, 55), tint(ACCENT, 120)),
        (&mut w.active, Color32::from_rgb(44, 40, 72), ACCENT),
        (&mut w.open, Color32::from_rgb(36, 41, 55), tint(ACCENT, 120)),
    ] {
        st.bg_fill = fill;
        st.weak_bg_fill = fill;
        st.bg_stroke = Stroke::new(1.0_f32, stroke);
        st.fg_stroke = Stroke::new(1.5_f32, TEXT);
        st.corner_radius = CornerRadius::same(8);
        st.expansion = 0.0;
    }
    ctx.set_visuals(v);
    ctx.style_mut(|s| {
        s.spacing.item_spacing = vec2(8.0, 8.0);
        s.spacing.button_padding = vec2(12.0, 5.0);
        s.spacing.interact_size.y = 26.0;
        s.spacing.combo_width = 200.0;
        s.text_styles.insert(TextStyle::Heading, FontId::proportional(24.0));
        s.text_styles.insert(TextStyle::Body, FontId::proportional(14.0));
        s.text_styles.insert(TextStyle::Button, FontId::proportional(14.0));
        s.text_styles.insert(TextStyle::Small, FontId::proportional(12.0));
        s.text_styles.insert(TextStyle::Monospace, FontId::monospace(13.0));
    });
}

pub fn card<R>(ui: &mut Ui, add: impl FnOnce(&mut Ui) -> R) -> R {
    egui::Frame::new()
        .fill(CARD)
        .stroke(Stroke::new(1.0_f32, BORDER))
        .corner_radius(CornerRadius::same(12))
        .inner_margin(Margin::same(16))
        .show(ui, |ui| {
            ui.set_width(ui.available_width());
            add(ui)
        })
        .inner
}

pub fn card_tinted<R>(ui: &mut Ui, color: Color32, add: impl FnOnce(&mut Ui) -> R) -> R {
    egui::Frame::new()
        .fill(tint(color, 18))
        .stroke(Stroke::new(1.0_f32, tint(color, 90)))
        .corner_radius(CornerRadius::same(12))
        .inner_margin(Margin::same(16))
        .show(ui, |ui| {
            ui.set_width(ui.available_width());
            add(ui)
        })
        .inner
}

pub fn page_header(ui: &mut Ui, title: &str, subtitle: &str) {
    ui.add_space(4.0);
    ui.label(RichText::new(title).size(26.0).strong());
    ui.label(RichText::new(subtitle).color(MUTED));
    ui.add_space(10.0);
}

pub fn section(ui: &mut Ui, title: &str) {
    ui.add_space(4.0);
    ui.label(RichText::new(title.to_uppercase()).size(11.5).color(FAINT).strong());
}

pub fn muted(text: impl Into<String>) -> RichText {
    RichText::new(text.into()).color(MUTED)
}

pub fn small(text: impl Into<String>) -> RichText {
    RichText::new(text.into()).size(12.0).color(MUTED)
}

/// Small coloured pill. Painted as a single widget so it wraps in `horizontal_wrapped`.
pub fn chip(ui: &mut Ui, text: impl Into<String>, color: Color32) -> Response {
    let galley = ui.painter().layout_no_wrap(text.into(), FontId::proportional(12.0), color);
    let size = galley.size() + vec2(16.0, 6.0);
    let (rect, resp) = ui.allocate_exact_size(size, Sense::hover());
    if ui.is_rect_visible(rect) {
        let p = ui.painter();
        p.rect_filled(rect, CornerRadius::same(10), tint(color, 28));
        p.rect_stroke(rect, CornerRadius::same(10), Stroke::new(1.0_f32, tint(color, 70)), StrokeKind::Inside);
        p.galley(pos2(rect.left() + 8.0, rect.center().y - galley.size().y / 2.0), galley, color);
    }
    resp
}

/// Removable list item (painted, wraps). Returns true when its x is clicked.
pub fn removable_chip(ui: &mut Ui, text: &str) -> bool {
    let galley = ui.painter().layout_no_wrap(text.to_string(), FontId::proportional(12.5), TEXT);
    let x_w = 18.0;
    let size = vec2(galley.size().x + 10.0 + x_w + 4.0, galley.size().y + 6.0);
    let (rect, resp) = ui.allocate_exact_size(size, Sense::click());
    let x_rect = egui::Rect::from_min_size(pos2(rect.right() - x_w - 3.0, rect.top()), vec2(x_w, rect.height()));
    let over_x = resp.hover_pos().is_some_and(|p| x_rect.contains(p));
    if ui.is_rect_visible(rect) {
        let p = ui.painter();
        p.rect_filled(rect, CornerRadius::same(10), CARD_HI);
        p.rect_stroke(rect, CornerRadius::same(10), Stroke::new(1.0_f32, if over_x { tint(BAD, 140) } else { BORDER }), StrokeKind::Inside);
        p.galley(pos2(rect.left() + 10.0, rect.center().y - galley.size().y / 2.0), galley, TEXT);
        let c = x_rect.center();
        let s = 3.5;
        let col = if over_x { BAD } else { MUTED };
        p.line_segment([c + vec2(-s, -s), c + vec2(s, s)], Stroke::new(1.4_f32, col));
        p.line_segment([c + vec2(-s, s), c + vec2(s, -s)], Stroke::new(1.4_f32, col));
    }
    let resp = if over_x { resp.on_hover_text("Remove") } else { resp };
    resp.clicked() && resp.interact_pointer_pos().is_some_and(|p| x_rect.contains(p))
}

pub fn primary(ui: &mut Ui, text: &str) -> Response {
    ui.add(egui::Button::new(RichText::new(text).color(Color32::WHITE).strong()).fill(ACCENT).stroke(Stroke::NONE))
}

pub fn danger(ui: &mut Ui, text: &str) -> Response {
    ui.add(egui::Button::new(RichText::new(text).color(BAD)).fill(tint(BAD, 22)).stroke(Stroke::new(1.0_f32, tint(BAD, 80))))
}

/// iOS-style switch.
pub fn toggle(ui: &mut Ui, on: &mut bool) -> Response {
    let size = vec2(38.0, 21.0);
    let (rect, mut resp) = ui.allocate_exact_size(size, Sense::click());
    if resp.clicked() {
        *on = !*on;
        resp.mark_changed();
    }
    if ui.is_rect_visible(rect) {
        let t = ui.ctx().animate_bool(resp.id, *on);
        let lerp = |a: u8, b: u8| (a as f32 + (b as f32 - a as f32) * t) as u8;
        let bg = Color32::from_rgb(lerp(52, ACCENT.r()), lerp(58, ACCENT.g()), lerp(74, ACCENT.b()));
        let p = ui.painter();
        p.rect_filled(rect, CornerRadius::same(11), bg);
        if resp.hovered() {
            p.rect_stroke(rect, CornerRadius::same(11), Stroke::new(1.0_f32, tint(Color32::WHITE, 60)), StrokeKind::Inside);
        }
        let x = egui::lerp((rect.left() + 10.5)..=(rect.right() - 10.5), t);
        p.circle_filled(pos2(x, rect.center().y), 7.5, Color32::WHITE);
    }
    resp
}

/// Toggle + label + optional hint, laid out as a settings row.
pub fn toggle_row(ui: &mut Ui, on: &mut bool, label: &str, hint: &str) -> bool {
    let mut changed = false;
    ui.horizontal(|ui| {
        changed = toggle(ui, on).changed();
        ui.vertical(|ui| {
            ui.spacing_mut().item_spacing.y = 1.0;
            ui.label(label);
            if !hint.is_empty() {
                ui.label(small(hint));
            }
        });
    });
    changed
}

/// Left-aligned, truncated label in a fixed-width cell.
pub fn cell(ui: &mut Ui, width: f32, height: f32, text: impl Into<egui::WidgetText>) -> Response {
    ui.allocate_ui_with_layout(vec2(width, height), egui::Layout::left_to_right(egui::Align::Center), |ui| {
        ui.set_min_size(vec2(width, height));
        ui.add(egui::Label::new(text).truncate())
    })
    .inner
}

pub fn status_dot(ui: &mut Ui, color: Color32, radius: f32) {
    let (rect, _) = ui.allocate_exact_size(vec2(radius * 2.0 + 4.0, radius * 2.0 + 4.0), Sense::hover());
    ui.painter().circle_filled(rect.center(), radius + 3.0, tint(color, 40));
    ui.painter().circle_filled(rect.center(), radius, color);
}

/// Simple filled sparkline.
pub fn sparkline(ui: &mut Ui, values: &[f32], max: f32, color: Color32, height: f32) {
    let (rect, _) = ui.allocate_exact_size(vec2(ui.available_width(), height), Sense::hover());
    let p = ui.painter_at(rect);
    p.rect_filled(rect, CornerRadius::same(6), tint(color, 10));
    if values.len() < 2 {
        return;
    }
    let max = values.iter().cloned().fold(max, f32::max).max(1.0);
    let n = values.len();
    let pts: Vec<egui::Pos2> = values
        .iter()
        .enumerate()
        .map(|(i, v)| {
            let x = rect.left() + rect.width() * i as f32 / (n - 1) as f32;
            let y = rect.bottom() - 2.0 - (rect.height() - 4.0) * (v / max).clamp(0.0, 1.0);
            pos2(x, y)
        })
        .collect();
    let mut fill = pts.clone();
    fill.push(pos2(rect.right(), rect.bottom()));
    fill.push(pos2(rect.left(), rect.bottom()));
    // fill as thin vertical bars (convex-safe)
    for w in pts.windows(2) {
        let quad = vec![w[0], w[1], pos2(w[1].x, rect.bottom()), pos2(w[0].x, rect.bottom())];
        p.add(egui::Shape::convex_polygon(quad, tint(color, 26), Stroke::NONE));
    }
    p.add(egui::Shape::line(pts, Stroke::new(1.6_f32, color)));
}

/// Thin progress bar.
pub fn bar(ui: &mut Ui, frac: f32, color: Color32) {
    let (rect, _) = ui.allocate_exact_size(vec2(ui.available_width(), 6.0), Sense::hover());
    ui.painter().rect_filled(rect, CornerRadius::same(3), CARD_HI);
    let mut r = rect;
    r.set_width(rect.width() * frac.clamp(0.0, 1.0));
    ui.painter().rect_filled(r, CornerRadius::same(3), color);
}

pub fn severity_color(load: f32) -> Color32 {
    if load >= 90.0 {
        BAD
    } else if load >= 70.0 {
        WARN
    } else {
        GOOD
    }
}
