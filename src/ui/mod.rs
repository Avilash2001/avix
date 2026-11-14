//! The dashboard window (egui). Spawned on demand by the engine; when it's
//! closed the whole process exits, so it costs nothing while you game.

mod dashboard;
mod data;
mod debloat;
mod health;
mod processes;
mod profiles;
mod search;
mod settings;
#[cfg(debug_assertions)]
mod shot;
mod theme;

use crate::config::{Config, UiTheme, STAR_CARD_AFTER_DAYS};
use crate::engine::UI_TITLE;
use crate::icon::{render_rgba, IconState};
use crate::ipc::Request;
use eframe::egui::{
    self, pos2, vec2, Align, Align2, Color32, CornerRadius, Frame, Id, Key, Layout, Margin, Modifiers,
    Order, Rect, Sense, Stroke, Ui,
};
use std::time::{Duration, Instant};
use theme::*;

pub fn run() {
    let icon = render_rgba(64, IconState::Idle);
    #[allow(unused_mut)]
    let mut size = [1240.0, 800.0];
    #[cfg(debug_assertions)]
    if let Some(s) = shot::window_size() {
        size = s;
    }
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_title(UI_TITLE)
            .with_inner_size(size)
            .with_min_inner_size([680.0, 520.0])
            .with_icon(egui::IconData { rgba: icon, width: 64, height: 64 }),
        centered: true,
        ..Default::default()
    };
    if let Err(e) = eframe::run_native(UI_TITLE, options, Box::new(|cc| Ok(Box::new(App::new(cc))))) {
        crate::logging::log(&format!("UI failed to start: {e}"));
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug, Hash)]
pub enum Page {
    Dashboard,
    Profiles,
    Processes,
    Debloat,
    Health,
    Settings,
}

impl Page {
    const ALL: [Page; 6] = [Page::Dashboard, Page::Profiles, Page::Processes, Page::Debloat, Page::Health, Page::Settings];

    fn label(self) -> &'static str {
        match self {
            Page::Dashboard => "Dashboard",
            Page::Profiles => "Boost profiles",
            Page::Processes => "Processes",
            Page::Debloat => "Debloat & tweaks",
            Page::Health => "Health check",
            Page::Settings => "Settings",
        }
    }

    fn icon(self) -> char {
        match self {
            Page::Dashboard => icon::DASHBOARD,
            Page::Profiles => icon::ZAP,
            Page::Processes => icon::CPU,
            Page::Debloat => icon::SPARKLES,
            Page::Health => icon::HEART_PULSE,
            Page::Settings => icon::SETTINGS,
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum ToastKind {
    Ok,
    Err,
    Info,
}

struct Toast {
    text: String,
    kind: ToastKind,
    until: Instant,
    born: f64,
}

pub struct Confirm {
    pub title: String,
    pub body: String,
    pub ok: String,
    pub danger: bool,
    pub action: Box<dyn FnOnce(&mut App)>,
}

pub struct App {
    pub page: Page,
    page_since: f64,
    pub monitor: data::Monitor,
    pub cfg: Config,
    pub cfg_saved: Config,
    pub elevated: bool,
    ctx: egui::Context,
    logo: egui::TextureHandle,
    toasts: Vec<Toast>,
    pub confirm: Option<Confirm>,
    pub search: search::State,
    pub profiles: profiles::State,
    pub processes: processes::State,
    pub debloat: debloat::State,
    pub health: health::State,
    pub settings: settings::State,
    #[cfg(debug_assertions)]
    shot: Option<shot::Shot>,
}

impl App {
    fn new(cc: &eframe::CreationContext) -> Self {
        let cfg = Config::load();
        theme::setup(&cc.egui_ctx, cfg.animations);
        apply_theme(&cc.egui_ctx, cfg.theme);
        let ping = if cfg.welcomed { cfg.ping_host.clone() } else { String::new() };
        let monitor = data::Monitor::start(cc.egui_ctx.clone(), ping);
        let rgba = render_rgba(128, IconState::Idle);
        let logo = cc.egui_ctx.load_texture(
            "avix-logo",
            egui::ColorImage::from_rgba_unmultiplied([128, 128], &rgba),
            egui::TextureOptions::LINEAR,
        );
        let page = std::env::args()
            .find_map(|a| a.strip_prefix("--page=").map(str::to_lowercase))
            .and_then(|p| Page::ALL.into_iter().find(|x| format!("{x:?}").to_lowercase() == p))
            .unwrap_or(Page::Dashboard);
        let mut app = Self {
            page,
            page_since: -10.0,
            monitor,
            cfg_saved: cfg.clone(),
            cfg,
            elevated: crate::sys::elevation::is_elevated() || shot_admin(),
            ctx: cc.egui_ctx.clone(),
            logo,
            toasts: Vec::new(),
            confirm: None,
            search: Default::default(),
            profiles: Default::default(),
            processes: Default::default(),
            debloat: Default::default(),
            health: Default::default(),
            settings: Default::default(),
            #[cfg(debug_assertions)]
            shot: shot::Shot::from_env(),
        };
        app.note_open_day();
        app
    }

    /// Switch page (with the entrance transition).
    pub fn go(&mut self, page: Page) {
        if self.page != page {
            self.page = page;
            self.page_since = self.ctx.input(|i| i.time);
        }
    }

    pub fn toast(&mut self, kind: ToastKind, text: impl Into<String>) {
        let secs = if kind == ToastKind::Err { 7 } else { 4 };
        let born = self.ctx.input(|i| i.time);
        self.toasts.push(Toast { text: text.into(), kind, until: Instant::now() + Duration::from_secs(secs), born });
    }

    pub fn ask(&mut self, title: &str, body: &str, ok: &str, danger: bool, action: impl FnOnce(&mut App) + 'static) {
        self.confirm = Some(Confirm {
            title: title.into(),
            body: body.into(),
            ok: ok.into(),
            danger,
            action: Box::new(action),
        });
    }

    pub fn dirty(&self) -> bool {
        self.cfg != self.cfg_saved
    }

    pub fn save_config(&mut self) {
        self.cfg.normalize();
        match self.cfg.save() {
            Ok(()) => {
                self.cfg_saved = self.cfg.clone();
                self.monitor.send(Request::ReloadConfig);
                self.monitor.set_ping_host(&self.cfg.ping_host);
                self.toast(ToastKind::Ok, "Saved — the engine picked up your changes");
            }
            Err(e) => self.toast(ToastKind::Err, format!("Couldn't save settings: {e}")),
        }
    }

    /// Theme and animations are dashboard-only preferences: they apply and save at once, without
    /// touching any other unsaved edits.
    pub fn set_ui_prefs(&mut self, theme: Option<UiTheme>, animations: Option<bool>) {
        if let Some(t) = theme {
            apply_theme(&self.ctx, t);
        }
        self.save_dashboard_pref(|c| {
            if let Some(t) = theme {
                c.theme = t;
            }
            if let Some(a) = animations {
                c.animations = a;
            }
        });
    }

    /// Change a dashboard-only field on disk and in both in-memory copies, so it never shows up
    /// as an unsaved change and never saves anyone's pending edits along with it.
    fn save_dashboard_pref(&mut self, set: impl Fn(&mut Config)) {
        let mut disk = Config::load();
        set(&mut disk);
        set(&mut self.cfg);
        set(&mut self.cfg_saved);
        if let Err(e) = disk.save() {
            self.toast(ToastKind::Err, format!("Couldn't save the preference: {e}"));
        }
    }

    /// Count the days the dashboard is opened on, until the star card is due. Only after the
    /// first-run notice was answered, which also means config.json exists and parsed.
    fn note_open_day(&mut self) {
        let today = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_secs() / 86_400)
            .unwrap_or(0);
        let c = &self.cfg_saved;
        if !c.welcomed || c.star_card_done || c.open_days >= STAR_CARD_AFTER_DAYS || c.last_open_day == today {
            return;
        }
        let days = c.open_days + 1;
        self.save_dashboard_pref(|c| {
            c.open_days = days;
            c.last_open_day = today;
        });
    }

    /// The one-time "star Avix on GitHub" card on the dashboard.
    pub fn star_card_due(&self) -> bool {
        let c = &self.cfg_saved;
        c.welcomed && !c.star_card_done && c.open_days >= STAR_CARD_AFTER_DAYS
    }

    pub fn answer_star_card(&mut self, star: bool) {
        if star {
            crate::sys::elevation::open_url(crate::links::REPO_URL);
            self.toast(ToastKind::Ok, "Thank you! Every star helps other people find Avix");
        }
        self.save_dashboard_pref(|c| c.star_card_done = true);
    }

    /// Settings → About, outlined briefly (from the sidebar's heart).
    pub fn show_about(&mut self) {
        self.go(Page::Settings);
        self.settings.focus = Some(("about", self.ctx.input(|i| i.time)));
    }

    /// egui context for spawning jobs from confirm callbacks.
    pub fn monitor_ctx(&self) -> egui::Context {
        self.ctx.clone()
    }

    pub fn engine_status(&self) -> Option<crate::ipc::Status> {
        data::lock(&self.monitor.metrics).engine.clone()
    }

    pub fn start_engine(&mut self) {
        match std::env::current_exe().and_then(|exe| std::process::Command::new(exe).args(["--tray", "--no-elevate"]).spawn()) {
            Ok(_) => self.toast(ToastKind::Info, "Starting the engine…"),
            Err(e) => self.toast(ToastKind::Err, format!("Couldn't start the engine: {e}")),
        }
    }

    pub fn restart_engine(&mut self) {
        self.monitor.send(Request::Shutdown);
        std::thread::spawn(|| {
            std::thread::sleep(Duration::from_millis(900));
            if let Ok(exe) = std::env::current_exe() {
                let _ = std::process::Command::new(exe).args(["--tray", "--no-elevate"]).spawn();
            }
        });
        self.toast(ToastKind::Info, "Restarting the engine…");
    }

    fn restart_as_admin(&mut self, ctx: &egui::Context) {
        self.monitor.send(Request::Shutdown);
        std::thread::sleep(Duration::from_millis(400));
        if let Ok(exe) = std::env::current_exe() {
            if crate::sys::elevation::run_elevated(&exe.to_string_lossy(), "") {
                ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                return;
            }
        }
        self.start_engine();
        self.toast(ToastKind::Err, "Admin rights were declined");
    }

    // ------------------------------------------------------------ chrome

    fn nav(&mut self, ui: &mut Ui) {
        let ctx = ui.ctx().clone();
        ui.with_layout(Layout::top_down(Align::Center), |ui| {
            ui.add(egui::Image::new(&self.logo).fit_to_exact_size(vec2(34.0, 34.0)))
                .on_hover_text(format!("Avix {}", crate::VERSION));
            ui.add_space(22.0);

            let (item, gap, pad) = (40.0, 6.0, 6.0);
            let n = Page::ALL.len() as f32;
            // the capsule also holds its foot: a hairline, the theme switch, About and the engine dot
            let foot = 12.0 + 1.0 + 8.0 + item + 2.0 + item + 4.0 + 22.0;
            let (rect, _) = ui.allocate_exact_size(vec2(item + pad * 2.0, pad * 2.0 + n * item + (n - 1.0) * gap + foot), Sense::hover());
            let p = pal();
            ui.painter().rect_filled(rect, CornerRadius::same(((item + pad * 2.0) / 2.0) as u8), p.sunken);
            let idx = Page::ALL.iter().position(|x| *x == self.page).unwrap_or(0) as f32;
            let y = ease_value(&ctx, Id::new("nav-disc"), idx * (item + gap), 0.32);
            let disc = Rect::from_min_size(pos2(rect.left() + pad, rect.top() + pad + y), vec2(item, item));
            ui.painter().circle_filled(disc.center(), item / 2.0, p.accent);
            for (i, page) in Page::ALL.into_iter().enumerate() {
                let cell = Rect::from_min_size(pos2(rect.left() + pad, rect.top() + pad + i as f32 * (item + gap)), vec2(item, item));
                let resp = ui
                    .interact(cell, Id::new(("nav", i)), Sense::click())
                    .on_hover_cursor(egui::CursorIcon::PointingHand)
                    .on_hover_text(page.label());
                let selected = page == self.page;
                let hov = ease_bool(&ctx, resp.id.with("h"), resp.hovered() && !selected, 0.12);
                if hov > 0.0 {
                    ui.painter().circle_filled(cell.center(), item / 2.0, p.hover.gamma_multiply(hov));
                }
                let on_disc = (disc.center().y - cell.center().y).abs() < item * 0.5;
                let color = if on_disc { Color32::WHITE } else if resp.hovered() { p.text } else { p.muted };
                paint_icon(ui, cell.center(), page.icon(), 18.0, color);
                if resp.clicked() {
                    self.go(page);
                }
            }

            let foot_top = rect.top() + pad + n * item + (n - 1.0) * gap + 12.0;
            ui.painter().hline((rect.left() + 12.0)..=(rect.right() - 12.0), foot_top, Stroke::new(1.0_f32, p.border_strong));
            let (ic, next) = match self.cfg.theme {
                UiTheme::System => (icon::SUN_MOON, UiTheme::Light),
                UiTheme::Light => (icon::SUN, UiTheme::Dark),
                UiTheme::Dark => (icon::MOON, UiTheme::System),
            };
            let cell = Rect::from_min_size(pos2(rect.left() + pad, foot_top + 9.0), vec2(item, item));
            let resp = ui
                .interact(cell, Id::new("nav-theme"), Sense::click())
                .on_hover_cursor(egui::CursorIcon::PointingHand)
                .on_hover_text(format!("Theme: {} (click for {})", self.cfg.theme.label(), next.label()));
            let hov = ease_bool(&ctx, resp.id.with("h"), resp.hovered(), 0.12);
            if hov > 0.0 {
                ui.painter().circle_filled(cell.center(), item / 2.0, p.hover.gamma_multiply(hov));
            }
            paint_icon(ui, cell.center(), ic, 17.0, if resp.hovered() { p.text } else { p.muted });
            if resp.clicked() {
                self.set_ui_prefs(Some(next), None);
            }

            let cell = cell.translate(vec2(0.0, item + 2.0));
            let resp = ui
                .interact(cell, Id::new("nav-about"), Sense::click())
                .on_hover_cursor(egui::CursorIcon::PointingHand)
                .on_hover_text(format!("About Avix\nMade by {}. Star it, support it or contribute.", crate::links::AUTHOR));
            let hov = ease_bool(&ctx, resp.id.with("h"), resp.hovered(), 0.12);
            if hov > 0.0 {
                ui.painter().circle_filled(cell.center(), item / 2.0, p.hover.gamma_multiply(hov));
            }
            paint_icon(ui, cell.center(), icon::HEART, 17.0, if resp.hovered() { p.text } else { p.muted });
            if resp.clicked() {
                self.show_about();
            }

            let status = self.engine_status();
            let (dot, tip) = match &status {
                Some(s) if s.active.is_some() => (Tone::Good.solid(), "Engine running — boosting".to_string()),
                Some(s) if !s.elevated => (Tone::Warn.solid(), "Engine running in limited mode (no admin)".to_string()),
                Some(_) => (Tone::Accent.solid(), "Engine running".to_string()),
                None => (Tone::Bad.solid(), "Engine offline".to_string()),
            };
            let tip = match &status {
                Some(s) => format!("{tip}\n{} RAM — {:.2}% CPU", crate::sys::fmt_bytes(s.engine_ram), s.engine_cpu_percent),
                None => tip,
            };
            let dot_rect = Rect::from_center_size(pos2(rect.center().x, cell.bottom() + 4.0 + 11.0), vec2(22.0, 22.0));
            ui.painter().circle_filled(dot_rect.center(), 7.0, dot.gamma_multiply(0.22));
            ui.painter().circle_filled(dot_rect.center(), 4.0, dot);
            ui.interact(dot_rect, Id::new("nav-engine-dot"), Sense::hover()).on_hover_text(tip);
        });
    }

    fn top_bar(&mut self, ui: &mut Ui) {
        let status = self.engine_status();
        ui.horizontal(|ui| {
            ui.set_height(36.0);
            ui.label(tw("avix", 21.0, W::Bold).extra_letter_spacing(-0.9));
            ui.add_space(20.0);
            let avail = ui.available_width();
            let search_w = (avail * 0.42).clamp(180.0, 400.0);
            search::field(self, ui, search_w);
            if avail < 640.0 {
                return;
            }
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                match &status {
                    Some(s) => {
                        stat_pill(ui, Some(icon::GAUGE), &format!("Engine {} · {:.1}% CPU", crate::sys::fmt_bytes(s.engine_ram), s.engine_cpu_percent))
                            .on_hover_text(format!("The background engine's own footprint. Up {}.", dashboard::fmt_dur(s.uptime_secs)));
                        if !s.elevated {
                            chip_icon(ui, Some(icon::LOCK), "Limited mode", Tone::Warn);
                        }
                        if let Some(u) = &s.update.available {
                            if Btn::new(Kind::Secondary, &format!("Avix {} is out", u.version)).icon(icon::DOWNLOAD).small().show(ui).clicked() {
                                crate::sys::elevation::open_url(&u.url);
                            }
                        }
                    }
                    None => {
                        chip_icon(ui, Some(icon::CIRCLE_X), "Engine offline", Tone::Bad);
                    }
                }
            });
        });
    }

    /// First run: say plainly what Avix changes automatically and what it sends over the network,
    /// and ask before checking for updates. Nothing goes over the network until this is answered.
    fn welcome(&mut self, ctx: &egui::Context) {
        let mut answer: Option<bool> = None;
        let logo = self.logo.clone();
        let ping = if self.cfg.ping_host.is_empty() { "nothing".to_string() } else { self.cfg.ping_host.clone() };
        egui::Modal::new(Id::new("avix-welcome"))
            .backdrop_color(Color32::from_black_alpha(if is_dark() { 160 } else { 90 }))
            .frame(modal_frame())
            .show(ctx, |ui| {
                ui.set_max_width(560.0);
                ui.horizontal(|ui| {
                    ui.add(egui::Image::new(&logo).fit_to_exact_size(vec2(40.0, 40.0)));
                    ui.vertical(|ui| {
                        ui.spacing_mut().item_spacing.y = 0.0;
                        ui.label(tw("Welcome to Avix", 22.0, W::Semi).extra_letter_spacing(-0.4));
                        ui.label(muted_t("Here's exactly what Avix does on this PC, so there are no surprises."));
                    });
                });
                ui.add_space(14.0);
                ui.label(tw("Automatically, while a game, stream or AI app is running", 14.0, W::Semi));
                ui.add_space(2.0);
                for line in [
                    "Switches to its tuned power plan",
                    "Gives the game high priority and lowers background apps (the window you're using is never slowed)",
                    "Pauses Windows Update and its download services",
                    "Frees memory when RAM runs low",
                    "Laptops with two GPUs: sets games to use the dedicated GPU",
                ] {
                    check_line(ui, line);
                }
                ui.add_space(6.0);
                ui.add(egui::Label::new(small(
                    "Everything goes back exactly as it was when the game closes — even if Avix is closed or crashes. Tweaks and app removal only happen when you click them. Avix never touches game memory and installs no drivers. You can pause it any time from the tray.",
                )).wrap());
                ui.add_space(14.0);
                ui.label(tw("Network", 14.0, W::Semi));
                ui.add_space(2.0);
                check_line(ui, "No telemetry: nothing about you or your PC is ever sent.");
                check_line(ui, &format!("Latency monitor: while the dashboard is open, Avix pings {ping} (change or turn off in Settings)."));
                check_line(ui, "Updates: may Avix ask GitHub once a day whether a new version is out? One HTTPS request; nothing about your PC is sent. You can change this later in Settings.");
                ui.add_space(18.0);
                ui.horizontal(|ui| {
                    if ui.link(small("Privacy policy")).clicked() {
                        crate::sys::elevation::open_url(crate::links::PRIVACY_URL);
                    }
                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        if primary(ui, "Start and check daily").clicked() {
                            answer = Some(true);
                        }
                        if secondary(ui, "Start without update checks").clicked() {
                            answer = Some(false);
                        }
                    });
                });
            });
        if let Some(yes) = answer {
            self.cfg.check_updates = yes;
            self.cfg.welcomed = true;
            self.save_config();
        }
    }

    fn admin_banner(&mut self, ui: &mut Ui) {
        let ctx = ui.ctx().clone();
        card_tinted(ui, Tone::Warn, |ui| {
            split_row(
                ui,
                "admin-banner",
                160.0,
                |ui| {
                    ui.horizontal(|ui| {
                        icon_label(ui, icon::LOCK, 16.0, Tone::Warn.fg());
                        ui.label(strong("Running without administrator rights"));
                    });
                    ui.add(egui::Label::new(small("Tweaks, app removal, services and the standby cleaner are disabled until Avix restarts as admin.")).wrap());
                },
                |ui| {
                    if primary(ui, "Restart as admin").clicked() {
                        self.restart_as_admin(&ctx);
                    }
                },
            );
        });
        ui.add_space(14.0);
    }

    fn save_bar(&mut self, ctx: &egui::Context) {
        let k = ease_bool(ctx, Id::new("savebar-in"), true, 0.25);
        egui::Area::new(Id::new("avix-savebar"))
            .order(Order::Foreground)
            .anchor(Align2::CENTER_BOTTOM, [41.0, -22.0 + (1.0 - k) * 24.0])
            .show(ctx, |ui| {
                ui.multiply_opacity(k);
                float_frame().inner_margin(Margin { left: 18, right: 10, top: 10, bottom: 10 }).corner_radius(CornerRadius::same(26)).show(ui, |ui| {
                    ui.horizontal(|ui| {
                        status_dot(ui, accent(), 4.0);
                        ui.label(strong("Unsaved changes"));
                        if ctx.screen_rect().width() > 900.0 {
                            ui.label(muted_t("They take effect when you save."));
                        }
                        ui.add_space(16.0);
                        if ghost(ui, "Discard").clicked() {
                            self.cfg = self.cfg_saved.clone();
                        }
                        if Btn::new(Kind::Primary, "Save & apply").icon(icon::CHECK).show(ui).clicked() {
                            self.save_config();
                        }
                    });
                });
            });
        if ctx.input(|i| i.modifiers.command && i.key_pressed(Key::S)) {
            self.save_config();
        }
    }

    fn show_confirm(&mut self, ctx: &egui::Context) {
        let Some(c) = self.confirm.take() else { return };
        let mut result = None;
        let modal = egui::Modal::new(Id::new("avix-confirm"))
            .backdrop_color(Color32::from_black_alpha(if is_dark() { 160 } else { 90 }))
            .frame(modal_frame())
            .show(ctx, |ui| {
                ui.set_width(420.0);
                ui.horizontal(|ui| {
                    icon_badge(ui, if c.danger { icon::ALERT } else { icon::INFO }, if c.danger { Tone::Bad } else { Tone::Accent }, 34.0);
                    ui.add_space(4.0);
                    ui.label(tw(&c.title, 17.0, W::Semi).extra_letter_spacing(-0.2));
                });
                ui.add_space(8.0);
                ui.add(egui::Label::new(muted_t(&c.body)).wrap());
                ui.add_space(18.0);
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    let ok = if c.danger { Btn::new(Kind::Danger, &c.ok).show(ui) } else { primary(ui, &c.ok) };
                    if ok.clicked() {
                        result = Some(true);
                    }
                    if secondary(ui, "Cancel").clicked() {
                        result = Some(false);
                    }
                });
            });
        if modal.should_close() {
            result = result.or(Some(false));
        }
        match result {
            Some(true) => (c.action)(self),
            Some(false) => {}
            None => self.confirm = Some(c),
        }
    }

    fn show_toasts(&mut self, ctx: &egui::Context) {
        let now = Instant::now();
        self.toasts.retain(|t| t.until > now);
        if self.toasts.is_empty() {
            return;
        }
        let newest = self.toasts.iter().map(|t| t.born).fold(f64::MIN, f64::max);
        let rise = 1.0 - entrance(ctx, newest, 0.3);
        // sit above the save bar when it's showing
        let base = if self.dirty() { -88.0 } else { -22.0 };
        egui::Area::new(Id::new("avix-toasts"))
            .order(Order::Tooltip)
            .anchor(Align2::RIGHT_BOTTOM, [-22.0, base + rise * 16.0])
            .show(ctx, |ui| {
                ui.set_max_width(400.0);
                ui.with_layout(Layout::top_down(Align::Max), |ui| {
                for toast in &self.toasts {
                    let k = entrance(ctx, toast.born, 0.3);
                    let (ic, tone) = match toast.kind {
                        ToastKind::Ok => (icon::CIRCLE_CHECK, Tone::Good),
                        ToastKind::Err => (icon::CIRCLE_ALERT, Tone::Bad),
                        ToastKind::Info => (icon::INFO, Tone::Accent),
                    };
                    ui.scope(|ui| {
                        ui.multiply_opacity(k);
                        float_frame().show(ui, |ui| {
                            // explicit left-to-right (the right-aligned stack would reverse `horizontal`),
                            // one width for every toast so the stack lines up
                            ui.allocate_ui_with_layout(vec2(340.0, 0.0), Layout::left_to_right(Align::Min), |ui| {
                                icon_label(ui, ic, 16.0, tone.fg());
                                ui.add(egui::Label::new(t(&toast.text)).wrap());
                            });
                        });
                    });
                    ui.add_space(8.0);
                }
                });
            });
        ctx.request_repaint_after(Duration::from_millis(500));
    }

    fn shortcuts(&mut self, ctx: &egui::Context) {
        let jump = ctx.input_mut(|i| {
            for (n, key) in [Key::Num1, Key::Num2, Key::Num3, Key::Num4, Key::Num5, Key::Num6].into_iter().enumerate() {
                if i.consume_key(Modifiers::COMMAND, key) {
                    return Some(Page::ALL[n]);
                }
            }
            None
        });
        if let Some(p) = jump {
            self.go(p);
        }
    }
}

/// Debug screenshots only (AVIX_SHOT_ADMIN=1): render as the normal, elevated app does.
pub fn shot_admin() -> bool {
    cfg!(debug_assertions) && std::env::var_os("AVIX_SHOT_ADMIN").is_some()
}

fn apply_theme(ctx: &egui::Context, t: UiTheme) {
    ctx.set_theme(match t {
        UiTheme::System => egui::ThemePreference::System,
        UiTheme::Light => egui::ThemePreference::Light,
        UiTheme::Dark => egui::ThemePreference::Dark,
    });
}

fn modal_frame() -> Frame {
    Frame::new()
        .fill(surface())
        .stroke(Stroke::new(1.0_f32, border()))
        .corner_radius(CornerRadius::same(20))
        .inner_margin(Margin::same(24))
        .shadow(egui::Shadow { offset: [0, 16], blur: 40, spread: 0, color: pal().shadow })
}

fn check_line(ui: &mut Ui, text: &str) {
    ui.horizontal_top(|ui| {
        ui.add_space(2.0);
        icon_label(ui, icon::CHECK, 14.0, Tone::Good.fg());
        ui.add(egui::Label::new(t(text)).wrap());
    });
}

/// Small rounded read-out used in the top bar.
fn stat_pill(ui: &mut Ui, ic: Option<char>, text: &str) -> egui::Response {
    let g = ui.painter().layout_no_wrap(text.to_string(), font(12.5, W::Medium), muted());
    let iw = if ic.is_some() { 20.0 } else { 0.0 };
    let (rect, resp) = ui.allocate_exact_size(vec2(g.size().x + 24.0 + iw, 30.0), Sense::hover());
    ui.painter().rect_filled(rect, CornerRadius::same(15), sunken());
    if let Some(c) = ic {
        paint_icon(ui, pos2(rect.left() + 12.0 + 7.0, rect.center().y), c, 14.0, muted());
    }
    ui.painter().galley(pos2(rect.left() + 12.0 + iw, rect.center().y - g.size().y / 2.0), g, muted());
    resp
}

impl eframe::App for App {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        theme::sync(ctx, self.cfg.animations);
        self.shortcuts(ctx);

        egui::SidePanel::left("nav")
            .exact_width(84.0)
            .resizable(false)
            .show_separator_line(false)
            .frame(Frame::new().fill(canvas()).inner_margin(Margin { left: 14, right: 14, top: 18, bottom: 18 }))
            .show(ctx, |ui| self.nav(ui));

        egui::TopBottomPanel::top("top")
            .exact_height(66.0)
            .show_separator_line(false)
            .frame(Frame::new().fill(canvas()).inner_margin(Margin { left: 4, right: 24, top: 16, bottom: 10 }))
            .show(ctx, |ui| self.top_bar(ui));

        let dirty = self.dirty();
        if dirty {
            // keep the floating save bar from covering content: the page ends above it
            egui::TopBottomPanel::bottom("savebar-space")
                .exact_height(76.0)
                .show_separator_line(false)
                .frame(Frame::new().fill(canvas()))
                .show(ctx, |_| {});
        }
        egui::CentralPanel::default()
            .frame(Frame::new().fill(canvas()).inner_margin(Margin { left: 4, right: 0, top: 4, bottom: 0 }))
            .show(ctx, |ui| {
                let k = entrance(ctx, self.page_since, 0.3);
                ui.multiply_opacity(k);
                if self.page == Page::Processes {
                    egui::Frame::new().inner_margin(Margin { left: 0, right: 24, top: 0, bottom: 16 }).show(ui, |ui| {
                        ui.add_space((1.0 - k) * 10.0);
                        if !self.elevated {
                            self.admin_banner(ui);
                        }
                        processes::show(self, ui);
                    });
                } else {
                    egui::ScrollArea::vertical().id_salt(self.page).auto_shrink([false; 2]).show(ui, |ui| {
                        egui::Frame::new().inner_margin(Margin { left: 0, right: 24, top: 0, bottom: 0 }).show(ui, |ui| {
                            ui.set_max_width(ui.available_width().min(1320.0));
                            ui.add_space((1.0 - k) * 10.0);
                            if !self.elevated {
                                self.admin_banner(ui);
                            }
                            match self.page {
                                Page::Dashboard => dashboard::show(self, ui),
                                Page::Profiles => profiles::show(self, ui),
                                Page::Debloat => debloat::show(self, ui),
                                Page::Health => health::show(self, ui),
                                Page::Settings => settings::show(self, ui),
                                Page::Processes => {}
                            }
                            ui.add_space(28.0);
                        });
                    });
                }
            });

        search::results(self, ctx);
        if dirty {
            self.save_bar(ctx);
        } else {
            ease_bool(ctx, Id::new("savebar-in"), false, 0.25);
        }
        if !self.cfg_saved.welcomed {
            self.welcome(ctx);
        }
        self.show_confirm(ctx);
        self.show_toasts(ctx);

        #[cfg(debug_assertions)]
        shot::step(self, ctx);
    }
}
