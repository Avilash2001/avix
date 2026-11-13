//! Top-bar search: pages, settings, tweaks and running processes. Arrow keys move, Enter opens,
//! Esc clears, Ctrl+K focuses the field from anywhere.

use super::data::lock;
use super::theme::*;
use super::{App, Page};
use crate::tweaks::catalog;
use eframe::egui::{self, pos2, vec2, Align2, CornerRadius, Id, Key, Modifiers, Order, Rect, Sense, Ui};

#[derive(Default)]
pub struct State {
    pub text: String,
    sel: usize,
    anchor: Option<Rect>,
    popup: Option<Rect>,
    focused: bool,
    pub field_id: Option<Id>,
}

#[derive(Clone)]
enum Hit {
    Page(Page),
    Setting(&'static str),
    Tweak(&'static str),
    Process(String),
}

struct Item {
    hit: Hit,
    title: String,
    sub: String,
    kind: &'static str,
    icon: char,
}

/// Settings sections: key (matched by settings.rs), title, extra keywords.
pub const SETTINGS: &[(&str, &str, &str)] = &[
    ("appearance", "Appearance", "theme dark light mode animations motion colour color"),
    ("general", "General", "start with windows autostart startup boot hotkey ctrl alt b notifications interval"),
    ("updates", "Updates", "update version github check download"),
    ("detect", "Auto-detect games", "fullscreen gpu library steam epic gog detect games ignore"),
    ("standby", "Standby memory cleaner", "standby memory ram cleaner islc free"),
    ("latency", "Latency monitor", "ping latency jitter network target server"),
    ("graphics", "Graphics", "gpu dedicated integrated laptop graphics hybrid"),
    ("safety", "Safety lists", "never touch anti-cheat anticheat hogs safety"),
    ("maintenance", "Maintenance", "data folder logs restart stop engine power plan reset defaults"),
    ("about", "About", "version about rust"),
];

fn matches(q: &str, hay: &str) -> bool {
    let hay = hay.to_lowercase();
    q.split_whitespace().all(|w| hay.contains(w))
}

fn items(app: &App, q: &str) -> Vec<Item> {
    let mut out = Vec::new();
    for p in Page::ALL {
        if matches(q, p.label()) {
            out.push(Item { hit: Hit::Page(p), title: p.label().into(), sub: "Go to page".into(), kind: "Page", icon: p.icon() });
        }
    }
    for (key, title, words) in SETTINGS {
        if matches(q, &format!("{title} {words}")) {
            out.push(Item { hit: Hit::Setting(key), title: (*title).into(), sub: "Settings".into(), kind: "Setting", icon: icon::SETTINGS });
        }
    }
    let mut tweaks: Vec<&crate::tweaks::Tweak> = catalog::all().iter().filter(|t| matches(q, t.title)).collect();
    tweaks.extend(catalog::all().iter().filter(|t| !matches(q, t.title) && matches(q, t.description)));
    for t in tweaks.into_iter().take(6) {
        out.push(Item { hit: Hit::Tweak(t.id), title: t.title.into(), sub: format!("Tweak · {}", t.category.label()), kind: "Tweak", icon: icon::SPARKLES });
    }
    let mut names: Vec<(bool, String)> = {
        let m = lock(&app.monitor.metrics);
        m.procs.iter().filter(|p| p.name_lc.contains(q)).map(|p| (!p.user_session, p.name.clone())).collect()
    };
    names.sort();
    let mut seen = std::collections::HashSet::new();
    names.retain(|(_, n)| seen.insert(n.to_lowercase()));
    for (_, n) in names.into_iter().take(5) {
        out.push(Item { hit: Hit::Process(n.clone()), title: n, sub: "Running process".into(), kind: "Process", icon: icon::CPU });
    }
    out.truncate(12);
    out
}

pub fn field(app: &mut App, ui: &mut Ui, width: f32) {
    let fid = ui.id().with(("field", "search"));
    app.search.field_id = Some(fid);
    if ui.input_mut(|i| i.consume_key(Modifiers::COMMAND, Key::K)) {
        ui.memory_mut(|m| m.request_focus(fid));
    }
    let hint = if width < 330.0 { "Search" } else { "Search tweaks, processes, settings" };
    let resp = text_field(ui, "search", &mut app.search.text, hint, width, Some(icon::SEARCH));
    let frame = Rect::from_min_max(resp.rect.min - vec2(38.0, 8.0), resp.rect.max + vec2(12.0, 8.0));
    app.search.anchor = Some(frame);
    app.search.focused = resp.has_focus();
    if resp.changed() {
        app.search.sel = 0;
    }
    if app.search.text.is_empty() && !resp.has_focus() {
        let g = ui.painter().layout_no_wrap("Ctrl K".into(), font(11.0, W::Medium), faint());
        let r = Rect::from_min_size(pos2(frame.right() - g.size().x - 22.0, frame.center().y - 10.0), vec2(g.size().x + 12.0, 20.0));
        ui.painter().rect_filled(r, CornerRadius::same(6), surface());
        ui.painter().galley(pos2(r.left() + 6.0, r.center().y - g.size().y / 2.0), g, faint());
    }
}

pub fn results(app: &mut App, ctx: &egui::Context) {
    let q = app.search.text.trim().to_lowercase();
    let hovering_popup = app.search.popup.zip(ctx.pointer_hover_pos()).is_some_and(|(r, p)| r.contains(p));
    let Some(anchor) = app.search.anchor else { return };
    if q.is_empty() || !(app.search.focused || hovering_popup) {
        app.search.popup = None;
        return;
    }
    let list = items(app, &q);
    let n = list.len();
    let mut open: Option<Hit> = None;
    if app.search.focused {
        ctx.input_mut(|i| {
            if i.consume_key(Modifiers::NONE, Key::ArrowDown) && n > 0 {
                app.search.sel = (app.search.sel + 1) % n;
            }
            if i.consume_key(Modifiers::NONE, Key::ArrowUp) && n > 0 {
                app.search.sel = (app.search.sel + n - 1) % n;
            }
            if i.key_pressed(Key::Enter) && n > 0 {
                open = Some(list[app.search.sel.min(n - 1)].hit.clone());
            }
        });
        if ctx.input(|i| i.key_pressed(Key::Escape)) {
            app.search.text.clear();
            if let Some(id) = app.search.field_id {
                ctx.memory_mut(|m| m.surrender_focus(id));
            }
            return;
        }
    }
    let width = anchor.width().max(380.0);
    let area = egui::Area::new(Id::new("avix-search-results"))
        .order(Order::Foreground)
        .fixed_pos(anchor.left_bottom() + vec2(0.0, 8.0))
        .show(ctx, |ui| {
            float_frame().inner_margin(egui::Margin::same(6)).show(ui, |ui| {
                ui.set_width(width - 12.0);
                if list.is_empty() {
                    ui.add_space(6.0);
                    ui.label(small(format!("  No matches for \"{}\"", app.search.text.trim())));
                    ui.add_space(6.0);
                    return;
                }
                ui.spacing_mut().item_spacing.y = 2.0;
                for (i, it) in list.iter().enumerate() {
                    let (rect, resp) = ui.allocate_exact_size(vec2(ui.available_width(), 44.0), Sense::click());
                    if resp.hovered() && ctx.input(|i| i.pointer.delta().length() > 0.0) {
                        app.search.sel = i;
                    }
                    let sel = i == app.search.sel.min(n - 1);
                    if sel {
                        ui.painter().rect_filled(rect, CornerRadius::same(10), sunken());
                    }
                    let c = pos2(rect.left() + 22.0, rect.center().y);
                    ui.painter().circle_filled(c, 15.0, if sel { surface() } else { sunken() });
                    paint_icon(ui, c, it.icon, 15.0, if sel { accent() } else { muted() });
                    let tx = rect.left() + 46.0;
                    let max_w = rect.width() - 46.0 - 80.0;
                    let mut job = egui::text::LayoutJob::simple_singleline(it.title.clone(), font(13.5, W::Medium), text());
                    job.wrap = egui::text::TextWrapping { max_width: max_w, max_rows: 1, break_anywhere: true, overflow_character: Some('\u{2026}') };
                    let title = ui.fonts(|f| f.layout_job(job));
                    ui.painter().galley(pos2(tx, rect.top() + 6.0), title, text());
                    ui.painter().text(pos2(tx, rect.bottom() - 7.0), Align2::LEFT_BOTTOM, &it.sub, font(11.5, W::Regular), muted());
                    ui.painter().text(pos2(rect.right() - 12.0, rect.center().y), Align2::RIGHT_CENTER, it.kind, font(11.0, W::Medium), faint());
                    if resp.on_hover_cursor(egui::CursorIcon::PointingHand).clicked() {
                        open = Some(it.hit.clone());
                    }
                }
            });
        });
    app.search.popup = Some(area.response.rect);
    if let Some(hit) = open {
        activate(app, ctx, hit);
    }
}

fn activate(app: &mut App, ctx: &egui::Context, hit: Hit) {
    let now = ctx.input(|i| i.time);
    match hit {
        Hit::Page(p) => app.go(p),
        Hit::Setting(key) => {
            app.go(Page::Settings);
            app.settings.focus = Some((key, now));
        }
        Hit::Tweak(id) => {
            if let Some(t) = catalog::find(id) {
                app.go(Page::Debloat);
                app.debloat.tab = super::debloat::Tab::Cat(t.category);
                app.debloat.focus = Some((id, now));
            }
        }
        Hit::Process(name) => {
            app.go(Page::Processes);
            app.processes.filter = name;
        }
    }
    app.search.text.clear();
    app.search.popup = None;
    if let Some(id) = app.search.field_id {
        ctx.memory_mut(|m| m.surrender_focus(id));
    }
}

/// Scrolls to a card that search jumped to and outlines it briefly in the accent colour.
pub fn flash(ui: &Ui, rect: Rect, focus: &mut Option<(&'static str, f64)>, key: &str) {
    let Some((k, since)) = *focus else { return };
    if k != key {
        return;
    }
    let now = ui.ctx().input(|i| i.time);
    let age = (now - since) as f32;
    if age < 0.05 {
        ui.scroll_to_rect(rect, Some(egui::Align::TOP));
    }
    let fade = 1.0 - (age / 1.6).clamp(0.0, 1.0);
    if fade <= 0.0 {
        *focus = None;
        return;
    }
    ui.painter().rect_stroke(rect.expand(1.0), CornerRadius::same(17), egui::Stroke::new(2.0_f32, accent().gamma_multiply(fade)), egui::StrokeKind::Outside);
    ui.ctx().request_repaint();
}
