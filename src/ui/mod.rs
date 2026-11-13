//! The dashboard window (egui). Spawned on demand by the engine; when it's
//! closed the whole process exits, so it costs nothing while you game.

mod dashboard;
mod data;
mod debloat;
mod health;
mod processes;
mod profiles;
mod settings;
mod theme;

use crate::config::Config;
use crate::engine::UI_TITLE;
use crate::icon::{render_rgba, IconState};
use crate::ipc::Request;
use eframe::egui::{
    self, pos2, vec2, Align, Align2, Color32, CornerRadius, FontId, Id, Layout, Margin, Order,
    RichText, Sense, Stroke, Ui,
};
use std::time::{Duration, Instant};
use theme::*;

pub fn run() {
    let icon = render_rgba(64, IconState::Idle);
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_title(UI_TITLE)
            .with_inner_size([1200.0, 780.0])
            .with_min_inner_size([980.0, 640.0])
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

    fn icon(self) -> &'static str {
        match self {
            Page::Dashboard => "⚡",
            Page::Profiles => "🎮",
            Page::Processes => "☰",
            Page::Debloat => "🔧",
            Page::Health => "✚",
            Page::Settings => "⚙",
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
    pub monitor: data::Monitor,
    pub cfg: Config,
    pub cfg_saved: Config,
    pub elevated: bool,
    ctx: egui::Context,
    logo: egui::TextureHandle,
    toasts: Vec<Toast>,
    pub confirm: Option<Confirm>,
    pub profiles: profiles::State,
    pub processes: processes::State,
    pub debloat: debloat::State,
    pub health: health::State,
    pub settings: settings::State,
}

impl App {
    fn new(cc: &eframe::CreationContext) -> Self {
        theme::apply(&cc.egui_ctx);
        let cfg = Config::load();
        let ping = if cfg.welcomed { cfg.ping_host.clone() } else { String::new() };
        let monitor = data::Monitor::start(cc.egui_ctx.clone(), ping);
        let rgba = render_rgba(64, IconState::Idle);
        let logo = cc.egui_ctx.load_texture(
            "avix-logo",
            egui::ColorImage::from_rgba_unmultiplied([64, 64], &rgba),
            egui::TextureOptions::LINEAR,
        );
        let page = std::env::args()
            .find_map(|a| a.strip_prefix("--page=").map(str::to_lowercase))
            .and_then(|p| Page::ALL.into_iter().find(|x| format!("{x:?}").to_lowercase() == p))
            .unwrap_or(Page::Dashboard);
        Self {
            page,
            monitor,
            cfg_saved: cfg.clone(),
            cfg,
            elevated: crate::sys::elevation::is_elevated(),
            ctx: cc.egui_ctx.clone(),
            logo,
            toasts: Vec::new(),
            confirm: None,
            profiles: Default::default(),
            processes: Default::default(),
            debloat: Default::default(),
            health: Default::default(),
            settings: Default::default(),
        }
    }

    pub fn toast(&mut self, kind: ToastKind, text: impl Into<String>) {
        let secs = if kind == ToastKind::Err { 7 } else { 4 };
        self.toasts.push(Toast { text: text.into(), kind, until: Instant::now() + Duration::from_secs(secs) });
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
                self.toast(ToastKind::Ok, "Saved - the engine picked up your changes");
            }
            Err(e) => self.toast(ToastKind::Err, format!("Couldn't save settings: {e}")),
        }
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
            Ok(_) => self.toast(ToastKind::Info, "Starting the engine..."),
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
        self.toast(ToastKind::Info, "Restarting the engine...");
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

    fn nav_item(ui: &mut Ui, page: Page, selected: bool) -> egui::Response {
        let (rect, resp) = ui.allocate_exact_size(vec2(ui.available_width(), 38.0), Sense::click());
        let p = ui.painter();
        let bg = if selected {
            tint(ACCENT, 38)
        } else if resp.hovered() {
            CARD_HI
        } else {
            Color32::TRANSPARENT
        };
        p.rect_filled(rect, CornerRadius::same(9), bg);
        if selected {
            p.rect_filled(
                egui::Rect::from_min_size(pos2(rect.left(), rect.top() + 10.0), vec2(3.0, 18.0)),
                CornerRadius::same(2),
                ACCENT,
            );
        }
        let color = if selected { TEXT } else { MUTED };
        p.text(rect.left_center() + vec2(16.0, 0.0), Align2::LEFT_CENTER, page.icon(), FontId::proportional(16.0), if selected { ACCENT } else { MUTED });
        p.text(rect.left_center() + vec2(44.0, 0.0), Align2::LEFT_CENTER, page.label(), FontId::proportional(14.5), color);
        resp
    }

    fn sidebar(&mut self, ui: &mut Ui) {
        ui.add_space(6.0);
        ui.horizontal(|ui| {
            ui.add(egui::Image::new(&self.logo).fit_to_exact_size(vec2(34.0, 34.0)));
            ui.vertical(|ui| {
                ui.spacing_mut().item_spacing.y = 0.0;
                ui.label(RichText::new("AVIX").size(20.0).strong());
                ui.label(RichText::new("Performance optimizer").size(11.5).color(MUTED));
            });
        });
        ui.add_space(18.0);
        for page in Page::ALL {
            if Self::nav_item(ui, page, self.page == page).clicked() {
                self.page = page;
            }
            ui.add_space(2.0);
        }

        let status = self.engine_status();
        ui.with_layout(Layout::bottom_up(Align::LEFT), |ui| {
            ui.add_space(4.0);
            ui.label(RichText::new(format!("v{}", crate::VERSION)).size(11.0).color(FAINT));
            egui::Frame::new()
                .fill(CARD)
                .stroke(Stroke::new(1.0_f32, BORDER))
                .corner_radius(CornerRadius::same(10))
                .inner_margin(Margin::same(10))
                .show(ui, |ui| {
                    ui.set_width(ui.available_width());
                    match &status {
                        Some(s) => {
                            ui.horizontal(|ui| {
                                status_dot(ui, if s.active.is_some() { GOOD } else { ACCENT }, 4.0);
                                ui.label(RichText::new("Engine running").size(13.0).strong());
                            });
                            ui.label(small(format!(
                                "{} RAM - {:.1}% CPU",
                                crate::sys::fmt_bytes(s.engine_ram),
                                s.engine_cpu_percent
                            )));
                            if !s.elevated {
                                ui.label(RichText::new("Limited mode (no admin)").size(11.5).color(WARN));
                            }
                        }
                        None => {
                            ui.horizontal(|ui| {
                                status_dot(ui, BAD, 4.0);
                                ui.label(RichText::new("Engine offline").size(13.0).strong());
                            });
                            if ui.small_button("Start engine").clicked() {
                                self.start_engine();
                            }
                        }
                    }
                });
        });
    }

    /// First run: say plainly what Avix changes automatically and what it sends over the network,
    /// and ask before checking for updates. Nothing goes over the network until this is answered.
    fn welcome(&mut self, ctx: &egui::Context) {
        let mut answer: Option<bool> = None;
        egui::Modal::new(egui::Id::new("avix-welcome"))
            .frame(egui::Frame::new().fill(CARD).stroke(Stroke::new(1.0_f32, BORDER)).corner_radius(10).inner_margin(Margin::same(22)))
            .show(ctx, |ui| {
                ui.set_max_width(560.0);
                ui.label(RichText::new("Welcome to Avix").size(22.0).strong());
                ui.label(muted("Here's exactly what Avix does on this PC, so there are no surprises."));
                ui.add_space(10.0);
                ui.label(RichText::new("Automatically, while a game, stream or AI app is running").strong());
                for line in [
                    "Switches to its tuned power plan",
                    "Gives the game high priority and lowers background apps (the window you're using is never slowed)",
                    "Pauses Windows Update and its download services",
                    "Frees memory when RAM runs low",
                    "Laptops with two GPUs: sets games to use the dedicated GPU",
                ] {
                    ui.label(format!("  -  {line}"));
                }
                ui.label(small(
                    "Everything goes back exactly as it was when the game closes - even if Avix is closed or crashes. Tweaks and app removal only happen when you click them. Avix never touches game memory and installs no drivers. You can pause it any time from the tray.",
                ));
                ui.add_space(10.0);
                ui.label(RichText::new("Network").strong());
                ui.label("  -  No telemetry: nothing about you or your PC is ever sent.");
                ui.label(format!(
                    "  -  Latency monitor: while the dashboard is open, Avix pings {} (change or turn off in Settings).",
                    if self.cfg.ping_host.is_empty() { "nothing" } else { self.cfg.ping_host.as_str() }
                ));
                ui.label("  -  Updates: may Avix ask GitHub once a day whether a new version is out? (One HTTPS request; nothing about your PC is sent. You can change this later in Settings.)");
                ui.add_space(14.0);
                ui.horizontal(|ui| {
                    if primary(ui, "Yes, check for updates").clicked() {
                        answer = Some(true);
                    }
                    if ui.button("No thanks").clicked() {
                        answer = Some(false);
                    }
                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        if ui.link("Privacy policy").clicked() {
                            crate::sys::elevation::open_url("https://github.com/Avilash2001/avix/blob/main/PRIVACY.md");
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
        card_tinted(ui, WARN, |ui| {
            ui.horizontal(|ui| {
                ui.label(RichText::new("!").strong().color(WARN));
                ui.label("Avix is running without administrator rights - tweaks, app removal, services and the standby cleaner are disabled.");
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    if primary(ui, "Restart as admin").clicked() {
                        self.restart_as_admin(&ctx);
                    }
                });
            });
        });
        ui.add_space(10.0);
    }

    fn save_bar(&mut self, ctx: &egui::Context) {
        egui::TopBottomPanel::bottom("savebar")
            .frame(egui::Frame::new().fill(Color32::from_rgb(24, 22, 40)).inner_margin(Margin::symmetric(24, 10)).stroke(Stroke::new(1.0_f32, tint(ACCENT, 90))))
            .show(ctx, |ui| {
                ui.horizontal(|ui| {
                    ui.label(RichText::new("You have unsaved changes").strong());
                    ui.label(muted("They take effect as soon as you save."));
                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        if primary(ui, "Save & apply").clicked() {
                            self.save_config();
                        }
                        if ui.button("Discard").clicked() {
                            self.cfg = self.cfg_saved.clone();
                        }
                    });
                });
            });
    }

    fn show_confirm(&mut self, ctx: &egui::Context) {
        let Some(c) = self.confirm.take() else { return };
        let screen = ctx.screen_rect();
        egui::Area::new(Id::new("avix-dim")).order(Order::Middle).fixed_pos(screen.min).show(ctx, |ui| {
            ui.painter().rect_filled(screen, CornerRadius::ZERO, Color32::from_black_alpha(160));
            ui.allocate_rect(screen, Sense::click());
        });
        let mut result = None;
        egui::Window::new(RichText::new(&c.title).strong())
            .order(Order::Foreground)
            .collapsible(false)
            .resizable(false)
            .anchor(Align2::CENTER_CENTER, [0.0, 0.0])
            .default_width(440.0)
            .show(ctx, |ui| {
                ui.add_space(4.0);
                ui.label(&c.body);
                ui.add_space(14.0);
                ui.horizontal(|ui| {
                    let ok = if c.danger { danger(ui, &c.ok) } else { primary(ui, &c.ok) };
                    if ok.clicked() {
                        result = Some(true);
                    }
                    if ui.button("Cancel").clicked() {
                        result = Some(false);
                    }
                });
            });
        if ctx.input(|i| i.key_pressed(egui::Key::Escape)) {
            result = Some(false);
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
        egui::Area::new(Id::new("avix-toasts"))
            .order(Order::Tooltip)
            .anchor(Align2::RIGHT_BOTTOM, [-20.0, -20.0])
            .show(ctx, |ui| {
                ui.set_max_width(420.0);
                for t in &self.toasts {
                    let color = match t.kind {
                        ToastKind::Ok => GOOD,
                        ToastKind::Err => BAD,
                        ToastKind::Info => INFO,
                    };
                    egui::Frame::new()
                        .fill(Color32::from_rgb(24, 27, 36))
                        .stroke(Stroke::new(1.0_f32, tint(color, 120)))
                        .corner_radius(CornerRadius::same(10))
                        .inner_margin(Margin::symmetric(14, 10))
                        .show(ui, |ui| {
                            ui.horizontal(|ui| {
                                status_dot(ui, color, 4.0);
                                ui.label(&t.text);
                            });
                        });
                    ui.add_space(6.0);
                }
            });
        ctx.request_repaint_after(Duration::from_millis(500));
    }
}

impl eframe::App for App {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        egui::SidePanel::left("nav")
            .exact_width(222.0)
            .resizable(false)
            .frame(egui::Frame::new().fill(SIDEBAR).inner_margin(Margin::same(14)).stroke(Stroke::new(1.0_f32, BORDER)))
            .show(ctx, |ui| self.sidebar(ui));

        if self.dirty() {
            self.save_bar(ctx);
        }

        egui::CentralPanel::default()
            .frame(egui::Frame::new().fill(BG).inner_margin(Margin { left: 28, right: 20, top: 20, bottom: 12 }))
            .show(ctx, |ui| {
                if !self.elevated {
                    self.admin_banner(ui);
                }
                if self.page == Page::Processes {
                    processes::show(self, ui);
                } else {
                    egui::ScrollArea::vertical().id_salt(self.page).auto_shrink([false; 2]).show(ui, |ui| {
                        let w = ui.available_width().min(1180.0);
                        ui.set_max_width(w);
                        match self.page {
                            Page::Dashboard => dashboard::show(self, ui),
                            Page::Profiles => profiles::show(self, ui),
                            Page::Debloat => debloat::show(self, ui),
                            Page::Health => health::show(self, ui),
                            Page::Settings => settings::show(self, ui),
                            Page::Processes => {}
                        }
                        ui.add_space(24.0);
                    });
                }
            });

        if !self.cfg_saved.welcomed {
            self.welcome(ctx);
        }
        self.show_confirm(ctx);
        self.show_toasts(ctx);
    }
}
