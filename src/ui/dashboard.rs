use super::data::lock;
use super::theme::*;
use super::{App, Page, ToastKind};
use crate::ipc::{EventKind, Mode, Request, Status};
use crate::sys::fmt_bytes;
use eframe::egui::{self, pos2, vec2, Align, Align2, CornerRadius, Id, Layout, Rect, Sense, Stroke, StrokeKind, Ui};

pub fn fmt_dur(secs: u64) -> String {
    if secs >= 3600 {
        format!("{}h {:02}m", secs / 3600, secs / 60 % 60)
    } else if secs >= 60 {
        format!("{}m {:02}s", secs / 60, secs % 60)
    } else {
        format!("{secs}s")
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Metric {
    Cpu,
    Gpu,
    Memory,
    Ping,
}

struct Snap {
    engine: Option<Status>,
    cpu: f32,
    cpu_hist: Vec<f32>,
    mem: crate::sys::memory::MemStatus,
    mem_hist: Vec<f32>,
    gpu: Option<f32>,
    gpu_hist: Vec<f32>,
    vram: u64,
    top: Vec<super::data::ProcRow>,
    ping: Option<u32>,
    ping_hist: Vec<f32>,
    stats: super::data::PingStats,
    target: String,
}

fn snap(app: &App) -> Snap {
    let m = lock(&app.monitor.metrics);
    Snap {
        engine: m.engine.clone(),
        cpu: m.cpu,
        cpu_hist: m.cpu_hist.iter().copied().collect(),
        mem: m.mem,
        mem_hist: m.mem_hist.iter().copied().collect(),
        gpu: m.gpu,
        gpu_hist: m.gpu_hist.iter().copied().collect(),
        vram: m.vram_used,
        top: m.procs.iter().filter(|p| p.name != "System" && p.name != "Memory Compression").take(7).cloned().collect(),
        ping: m.ping_ms,
        ping_hist: m.ping_hist.iter().map(|p| p.map(|v| v as f32).unwrap_or(0.0)).collect(),
        stats: m.ping_stats(),
        target: m.ping_target.clone(),
    }
}

fn gb(b: u64) -> f64 {
    b as f64 / 1073741824.0
}

pub fn show(app: &mut App, ui: &mut Ui) {
    let s = snap(app);
    header(app, ui, &s);
    ui.add_space(18.0);

    let w = ui.available_width();
    if w >= 1040.0 {
        let right_w = (w * 0.31).clamp(330.0, 400.0);
        let left_w = w - right_w - 18.0;
        ui.horizontal_top(|ui| {
            ui.spacing_mut().item_spacing.x = 18.0;
            ui.allocate_ui_with_layout(vec2(left_w, 0.0), Layout::top_down(Align::Min), |ui| {
                ui.set_width(left_w);
                main_top(app, ui, &s);
                main_bottom(app, ui, &s);
            });
            ui.allocate_ui_with_layout(vec2(right_w, 0.0), Layout::top_down(Align::Min), |ui| {
                ui.set_width(right_w);
                side_column(app, ui, &s);
            });
        });
    } else {
        main_top(app, ui, &s);
        side_column(app, ui, &s);
        ui.add_space(14.0);
        main_bottom(app, ui, &s);
    }
}

/// The engine's reason often just restates the target ("x.exe is running"); show it only when it
/// adds something.
fn extra_reason(target: &str, reason: &str) -> Option<String> {
    let same = reason.eq_ignore_ascii_case(&format!("{target} is running"));
    (!same && !reason.is_empty()).then(|| reason.to_string())
}

// ---------------------------------------------------------------- header

fn header(app: &mut App, ui: &mut Ui, s: &Snap) {
    let (title, sub, tone) = match &s.engine {
        None => ("Engine offline".to_string(), "The background engine isn't running, so nothing is being optimized.".to_string(), Tone::Bad),
        Some(e) => match (&e.active, e.mode) {
            (Some(a), _) => {
                let target = if a.target.is_empty() { "your PC".to_string() } else { a.target.trim_end_matches(".exe").trim_end_matches(".EXE").to_string() };
                let mut sub = format!("{} profile  ·  {}", a.profile, fmt_dur(a.since_secs));
                if let Some(r) = extra_reason(&a.target, &a.reason) {
                    sub.push_str(&format!("  ·  {r}"));
                }
                (format!("Boosting {target}"), sub, Tone::Good)
            }
            (None, Mode::Off) => ("Auto-boost paused".into(), "Avix won't boost anything until you resume auto-boost.".into(), Tone::Neutral),
            (None, _) => ("Watching for games".into(), "Avix boosts the moment a game, stream or AI app starts, and puts everything back after.".into(), Tone::Accent),
        },
    };
    let narrow = ui.available_width() < 760.0;
    let title_block = |ui: &mut Ui| {
        ui.horizontal(|ui| {
            status_dot(ui, tone.solid(), 5.0);
            ui.add(egui::Label::new(tw(&title, 26.0, W::Semi).extra_letter_spacing(-0.6)).truncate());
        });
        ui.add(egui::Label::new(muted_t(&sub)).truncate());
    };
    let offline = s.engine.is_none();
    let action = |app: &mut App, ui: &mut Ui| {
        if offline {
            if Btn::new(Kind::Primary, "Start engine").icon(icon::PLAY).show(ui).clicked() {
                app.start_engine();
            }
        } else {
            mode_control(app, ui, s);
        }
    };
    if narrow {
        title_block(ui);
        ui.add_space(10.0);
        ui.horizontal(|ui| action(app, ui));
        return;
    }
    split_row(ui, "dash-header", if offline { 160.0 } else { 330.0 }, title_block, |ui| action(app, ui));
}

fn mode_control(app: &mut App, ui: &mut Ui, s: &Snap) {
    let Some(e) = &s.engine else { return };
    let sel = match e.mode {
        Mode::Auto => 0,
        Mode::On => 1,
        Mode::Off => 2,
    };
    let items = [(Some(icon::GAUGE), "Auto"), (Some(icon::ZAP), "Boost now"), (Some(icon::PAUSE), "Pause")];
    let (clicked, rect) = segmented(ui, "mode", &items, sel, false);
    let names: Vec<String> = app.cfg.profiles.iter().filter(|p| p.enabled).map(|p| p.name.clone()).collect();
    let popup = Id::new("boost-profile-picker");
    match clicked {
        Some(0) => app.monitor.send(Request::SetMode { mode: Mode::Auto, profile: None }),
        Some(2) => app.monitor.send(Request::SetMode { mode: Mode::Off, profile: None }),
        Some(1) if names.len() <= 1 => app.monitor.send(Request::SetMode { mode: Mode::On, profile: names.first().cloned() }),
        Some(1) => ui.memory_mut(|m| m.toggle_popup(popup)),
        _ => {}
    }
    let resp = ui.interact(rect, Id::new("mode-anchor"), Sense::hover());
    egui::popup_below_widget(ui, popup, &resp, egui::PopupCloseBehavior::CloseOnClick, |ui| {
        ui.set_min_width(200.0);
        ui.label(label_t("Boost with profile"));
        for n in names {
            if ui.button(&n).clicked() {
                app.monitor.send(Request::SetMode { mode: Mode::On, profile: Some(n) });
            }
        }
    });
}

// ---------------------------------------------------------------- main column

fn main_top(app: &mut App, ui: &mut Ui, s: &Snap) {
    if let Some(e) = &s.engine {
        if let Some(u) = &e.update.available {
            card_tinted(ui, Tone::Accent, |ui| {
                split_row(
                    ui,
                    "update",
                    130.0,
                    |ui| {
                        ui.label(tw(format!("Avix {} is available", u.version), 15.0, W::Semi));
                        ui.add(egui::Label::new(small(format!("You have {}. Download the new Avix.exe, exit Avix from the tray, then replace the old file — your settings stay.", e.update.current))).wrap());
                    },
                    |ui| {
                        if Btn::new(Kind::Outline, "Download").icon(icon::DOWNLOAD).show(ui).clicked() {
                            crate::sys::elevation::open_url(&u.url);
                        }
                    },
                );
            });
            ui.add_space(14.0);
        }
    }

    live_card(app, ui, s);
    ui.add_space(14.0);
}

fn main_bottom(app: &mut App, ui: &mut Ui, s: &Snap) {
    detected_games(app, ui, s);
    top_processes(app, ui, s);
}

fn live_card(app: &mut App, ui: &mut Ui, s: &Snap) {
    let id = Id::new("dash-metric");
    let mut metric = ui.data(|d| d.get_temp::<u8>(id)).unwrap_or(0);
    let ctx = ui.ctx().clone();
    card(ui, |ui| {
        let w = ui.available_width();
        let cols = if w >= 560.0 { 4 } else { 2 };
        let gap = 10.0;
        let tile_w = (w - gap * (cols as f32 - 1.0)) / cols as f32;
        let tile_h = 82.0;
        let used = s.mem.total.saturating_sub(s.mem.avail);
        // Live values update in place once a second: easing them would keep the window repainting
        // most of the time, which costs CPU while you game.
        let cpu_v = s.cpu;
        let gpu_v = s.gpu;
        let mem_v = gb(used) as f32;
        let (ping_txt, ping_tone) = match s.ping {
            Some(p) => (format!("{p} ms"), if p < 40 { None } else if p < 90 { Some(Tone::Warn) } else { Some(Tone::Bad) }),
            None if s.stats.samples > 0 => ("timeout".into(), Some(Tone::Bad)),
            None if s.target.is_empty() => ("off".into(), None),
            None => ("…".into(), None),
        };
        let tiles: [(Metric, char, &str, String, String, Option<egui::Color32>); 4] = [
            (Metric::Cpu, icon::CPU, "CPU", format!("{cpu_v:.0}%"), format!("{} logical cores", crate::sys::process::logical_cpus()), Some(load_color(s.cpu))),
            (
                Metric::Gpu,
                icon::LAYERS,
                "GPU",
                gpu_v.map(|g| format!("{g:.0}%")).unwrap_or_else(|| "n/a".into()),
                if s.gpu.is_some() { format!("VRAM {}", fmt_bytes(s.vram)) } else { "Counters unavailable".into() },
                s.gpu.map(load_color),
            ),
            (Metric::Memory, icon::MEMORY, "Memory", format!("{mem_v:.1} GB"), format!("{}% of {:.1} GB", s.mem.load, gb(s.mem.total)), Some(load_color(s.mem.load as f32))),
            (Metric::Ping, icon::WIFI, "Ping", ping_txt, format!("jitter {:.0} ms · loss {:.0}%", s.stats.jitter, s.stats.loss), ping_tone.map(|t| t.fg())),
        ];
        let origin = ui.cursor().min;
        let rows = (tiles.len() as f32 / cols as f32).ceil();
        let grid_h = rows * tile_h + (rows - 1.0) * gap;
        let (_, _) = ui.allocate_exact_size(vec2(w, grid_h), Sense::hover());
        let p = pal();
        let sel_rect = |i: usize| {
            let (c, r) = (i % cols, i / cols);
            Rect::from_min_size(pos2(origin.x + c as f32 * (tile_w + gap), origin.y + r as f32 * (tile_h + gap)), vec2(tile_w, tile_h))
        };
        // sliding selection
        let target = sel_rect(metric as usize);
        let sx = ease_value(&ctx, Id::new("dash-sel-x"), target.left() - origin.x, 0.28) + origin.x;
        let sy = ease_value(&ctx, Id::new("dash-sel-y"), target.top() - origin.y, 0.28) + origin.y;
        let sel = Rect::from_min_size(pos2(sx, sy), target.size());
        ui.painter().rect_filled(sel, CornerRadius::same(12), p.accent_soft);
        for (i, (_, ic, label, value, sub, color)) in tiles.iter().enumerate() {
            let r = sel_rect(i);
            let resp = ui.interact(r, Id::new(("metric-tile", i)), Sense::click()).on_hover_cursor(egui::CursorIcon::PointingHand);
            if resp.clicked() {
                metric = i as u8;
            }
            if resp.hovered() && metric as usize != i {
                ui.painter().rect_stroke(r, CornerRadius::same(12), Stroke::new(1.0_f32, p.border), StrokeKind::Inside);
            }
            let x = r.left() + 14.0;
            let head = if metric as usize == i { p.accent_text } else { p.muted };
            paint_icon(ui, pos2(x + 7.0, r.top() + 20.0), *ic, 14.0, head);
            ui.painter().text(pos2(x + 20.0, r.top() + 20.0), Align2::LEFT_CENTER, *label, font(12.5, W::Medium), head);
            ui.painter().text(pos2(x, r.top() + 46.0), Align2::LEFT_CENTER, value, font(24.0, W::Semi), color.unwrap_or(p.text));
            let sub_g = ui.painter().layout_no_wrap(sub.clone(), font(11.5, W::Regular), p.muted);
            let clip = Rect::from_min_max(pos2(x, r.top() + 58.0), pos2(r.right() - 10.0, r.bottom()));
            ui.painter().with_clip_rect(clip).galley(pos2(x, r.top() + 62.0), sub_g, p.muted);
        }
        ui.data_mut(|d| d.insert_temp(id, metric));
        ui.add_space(16.0);

        let h = if w >= 560.0 { 196.0 } else { 150.0 };
        let accent = p.accent;
        match tiles[metric as usize].0 {
            Metric::Cpu => area_chart(ui, "cpu", &s.cpu_hist, 100.0, accent, h, &|v| format!("{v:.0}%")),
            Metric::Gpu => {
                if s.gpu.is_some() {
                    area_chart(ui, "gpu", &s.gpu_hist, 100.0, accent, h, &|v| format!("{v:.0}%"))
                } else {
                    empty_state(ui, icon::LAYERS, "GPU counters unavailable", "Windows didn't expose GPU usage counters on this PC.");
                }
            }
            Metric::Memory => area_chart(ui, "mem", &s.mem_hist, 100.0, accent, h, &|v| format!("{v:.0}%")),
            Metric::Ping => {
                if s.target.is_empty() {
                    empty_state(ui, icon::WIFI, "Latency monitor is off", "Set a ping target in Settings to watch jitter and packet loss.");
                } else {
                    area_chart(ui, "ping", &s.ping_hist, 60.0, accent, h, &|v| format!("{v:.0} ms"))
                }
            }
        }

        // context line under the chart
        ui.add_space(8.0);
        match tiles[metric as usize].0 {
            Metric::Memory => {
                ui.horizontal_wrapped(|ui| {
                    match s.engine.as_ref().and_then(|e| e.standby.zip(e.free)) {
                        Some((standby, free)) => ui.label(small(format!("Standby {}  ·  Free {}", fmt_bytes(standby), fmt_bytes(free)))),
                        None => ui.label(small(format!("{}% in use", s.mem.load))),
                    };
                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        if Btn::new(Kind::Secondary, "Free standby memory")
                            .icon(icon::REFRESH)
                            .small()
                            .show(ui)
                            .on_hover_text("Empties the standby cache (safe; Windows re-reads from disk if needed)")
                            .clicked()
                        {
                            if s.engine.as_ref().is_some_and(|e| e.elevated) {
                                app.monitor.send(Request::PurgeStandby);
                                app.toast(ToastKind::Info, "Purging standby memory…");
                            } else {
                                app.toast(ToastKind::Err, "Needs the engine running as admin");
                            }
                        }
                    });
                });
            }
            Metric::Ping if !s.target.is_empty() => {
                ui.label(small(format!("Pinging {} once a second while the dashboard is open  ·  average {:.0} ms", s.target, s.stats.avg)));
            }
            Metric::Cpu => {
                ui.label(small("System-wide CPU load, sampled once a second while the dashboard is open."));
            }
            _ => {}
        }
    });
}

fn detected_games(app: &mut App, ui: &mut Ui, s: &Snap) {
    let Some(e) = &s.engine else { return };
    let profile_names: Vec<String> = app.cfg.profiles.iter().map(|p| p.name.clone()).collect();
    let undetected: Vec<String> = e
        .detected_games
        .iter()
        .filter(|g| !app.cfg.profiles.iter().any(|p| p.triggers.iter().any(|t| t.eq_ignore_ascii_case(g))))
        .cloned()
        .collect();
    if undetected.is_empty() {
        return;
    }
    card_tinted(ui, Tone::Accent, |ui| {
        ui.horizontal(|ui| {
            icon_label(ui, icon::GAMEPAD, 16.0, Tone::Accent.fg());
            section(ui, "New games detected");
        });
        ui.add(egui::Label::new(small("Avix spotted these on its own (fullscreen or heavy GPU use). Add them to a profile so they boost the moment they launch.")).wrap());
        ui.add_space(6.0);
        for g in undetected {
            ui.horizontal_wrapped(|ui| {
                ui.label(strong(&g));
                for (i, n) in profile_names.iter().enumerate() {
                    if Btn::new(Kind::Outline, &format!("Add to {n}")).icon(icon::PLUS).small().show(ui).clicked() {
                        app.cfg.profiles[i].triggers.push(g.to_lowercase());
                        app.save_config();
                    }
                }
            });
        }
    });
    ui.add_space(14.0);
}

/// A neutral disc with the app's initial: gives lists a visual anchor without spending colour.
pub fn app_avatar(ui: &mut Ui, name: &str, size: f32) {
    let (rect, _) = ui.allocate_exact_size(vec2(size, size), Sense::hover());
    let initial = name.chars().find(|c| c.is_alphanumeric()).map(|c| c.to_ascii_uppercase()).unwrap_or('?');
    ui.painter().circle_filled(rect.center(), size / 2.0, pal().hover);
    ui.painter().text(rect.center(), Align2::CENTER_CENTER, initial, font(size * 0.42, W::Semi), muted());
}

fn top_processes(app: &mut App, ui: &mut Ui, s: &Snap) {
    card(ui, |ui| {
        section_with(ui, "Top processes", |ui| {
            if Btn::new(Kind::Ghost, "See all").icon(icon::ARROW_RIGHT).small().show(ui).clicked() {
                app.go(Page::Processes);
            }
        });
        ui.add_space(6.0);
        let w = ui.available_width();
        let (cpu_w, mem_w) = (64.0, 84.0);
        let name_w = (w - cpu_w - mem_w - 92.0).max(80.0);
        ui.horizontal(|ui| {
            ui.add_space(36.0);
            cell(ui, name_w, 18.0, label_t("Name"));
            cell_right(ui, cpu_w, 18.0, label_t("CPU"));
            cell_right(ui, mem_w, 18.0, label_t("Memory"));
        });
        for p in &s.top {
            divider(ui);
            ui.horizontal(|ui| {
                ui.set_height(34.0);
                app_avatar(ui, &p.name, 26.0);
                ui.add_space(2.0);
                cell(ui, name_w, 30.0, t(&p.name));
                cell_right(ui, cpu_w, 30.0, tw(format!("{:.1}%", p.cpu), 13.5, W::Medium).color(if p.cpu > 10.0 { Tone::Warn.fg() } else { text() }));
                cell_right(ui, mem_w, 30.0, muted_t(fmt_bytes(p.mem)));
            });
        }
    });
}

// ---------------------------------------------------------------- side column

fn side_column(app: &mut App, ui: &mut Ui, s: &Snap) {
    boost_card(app, ui, s);
    ui.add_space(14.0);
    activity(ui, s);
}

fn change_row(ui: &mut Ui, ic: char, label: &str, value: &str) -> egui::Response {
    // a long value (e.g. "Balanced → Avix Ultimate Performance") gets its own line instead of an ellipsis
    let measure = |s: &str, w: W| ui.painter().layout_no_wrap(s.to_string(), font(13.5, w), text()).size().x;
    if measure(label, W::Regular) + measure(value, W::Medium) + 15.0 + 40.0 > ui.available_width() {
        return ui
            .vertical(|ui| {
                ui.spacing_mut().item_spacing.y = 2.0;
                ui.horizontal(|ui| {
                    icon_label(ui, ic, 15.0, muted());
                    ui.label(muted_t(label));
                });
                ui.horizontal(|ui| {
                    ui.add_space(25.0);
                    ui.add(egui::Label::new(strong(value)).wrap());
                });
                ui.add_space(4.0);
            })
            .response;
    }
    ui.horizontal(|ui| {
        ui.set_height(26.0);
        icon_label(ui, ic, 15.0, muted());
        ui.label(muted_t(label));
        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
            ui.add(egui::Label::new(strong(value)).truncate());
        });
    })
    .response
}

fn boost_card(app: &mut App, ui: &mut Ui, s: &Snap) {
    let hotkey = app.cfg.hotkey;
    match &s.engine {
        None => card(ui, |ui| {
            icon_badge(ui, icon::POWER, Tone::Bad, 40.0);
            ui.add_space(6.0);
            ui.label(tw("Nothing is being boosted", 17.0, W::Semi));
            ui.add(egui::Label::new(muted_t("The engine runs in the tray using a few MB of RAM and boosts games automatically. Start it from the button at the top.")).wrap());
        }),
        Some(e) => match (&e.active, e.mode) {
            (Some(a), mode) => card_tinted(ui, Tone::Good, |ui| {
                let target = if a.target.is_empty() { "Whole system".to_string() } else { a.target.clone() };
                split_row(
                    ui,
                    "boost-head",
                    90.0,
                    |ui| {
                        ui.add(egui::Label::new(tw(&target, 18.0, W::Semi).extra_letter_spacing(-0.3)).truncate());
                        ui.label(muted_t(format!("{} profile", a.profile)));
                    },
                    |ui| {
                        ui.label(tw(fmt_dur(a.since_secs), 14.0, W::Medium).color(Tone::Good.fg()));
                    },
                );
                ui.add_space(12.0);
                ui.label(label_t("While boosting"));
                ui.add_space(2.0);
                let n = |v: u32, one: &str, many: &str| if v == 1 { format!("1 {one}") } else { format!("{v} {many}") };
                if !a.power_plan.is_empty() {
                    let v = if a.prev_power_plan.is_empty() { format!("{} (kept)", a.power_plan) } else { format!("{} → {}", a.prev_power_plan, a.power_plan) };
                    change_row(ui, icon::GAUGE, "Power plan", &v);
                }
                if a.throttled > 0 {
                    change_row(ui, icon::ARROW_DOWN, &format!("Background apps ({})", a.throttled), "Normal → Throttled");
                } else {
                    change_row(ui, icon::ARROW_DOWN, "Background apps", "Left alone");
                }
                if a.raised > 0 {
                    change_row(ui, icon::ARROW_UP, "Raised priority", &n(a.raised, "app", "apps"));
                }
                if !a.services_paused.is_empty() {
                    change_row(ui, icon::PAUSE, &format!("Services ({})", a.services_paused.len()), "Running → Paused").on_hover_text(a.services_paused.join(", "));
                }
                if a.timer_resolution {
                    change_row(ui, icon::TIMER, "Timer resolution", "Default → 0.5 ms");
                }
                if a.standby_purges > 0 {
                    change_row(ui, icon::MEMORY, "Standby memory", &format!("Purged {}", n(a.standby_purges, "time", "times")));
                }
                if a.apps_closed > 0 {
                    change_row(ui, icon::X, "Closed", &n(a.apps_closed, "app", "apps"));
                }
                if a.protected_target {
                    change_row(ui, icon::SHIELD_CHECK, "Game process", "Untouched (anti-cheat)");
                }
                ui.add_space(8.0);
                ui.horizontal_top(|ui| {
                    icon_label(ui, icon::UNDO, 14.0, Tone::Good.fg());
                    ui.add(egui::Label::new(small("Everything goes back exactly as it was when the app closes, even after a crash.")).wrap());
                });
                ui.add_space(10.0);
                let (label, to, tip) = if mode == Mode::On {
                    ("End boost", Mode::Auto, "Restores everything now and goes back to automatic boosting.")
                } else {
                    ("Pause boosting", Mode::Off, "Restores everything now. Auto-boost stays paused until you resume it.")
                };
                if Btn::new(Kind::Outline, label).icon(icon::PAUSE).show(ui).on_hover_text(tip).clicked() {
                    app.monitor.send(Request::SetMode { mode: to, profile: None });
                }
            }),
            (None, Mode::Off) => card(ui, |ui| {
                icon_badge(ui, icon::PAUSE, Tone::Neutral, 40.0);
                ui.add_space(6.0);
                ui.label(tw("Auto-boost is paused", 17.0, W::Semi));
                ui.add(egui::Label::new(muted_t("Games won't be boosted until you resume. Nothing Avix changed is left applied.")).wrap());
                ui.add_space(10.0);
                if Btn::new(Kind::Primary, "Resume auto-boost").icon(icon::PLAY).show(ui).clicked() {
                    app.monitor.send(Request::SetMode { mode: Mode::Auto, profile: None });
                }
            }),
            (None, _) => card(ui, |ui| {
                icon_badge(ui, icon::ROCKET, Tone::Accent, 40.0);
                ui.add_space(6.0);
                ui.label(tw("Ready to boost", 17.0, W::Semi));
                ui.add(egui::Label::new(muted_t("When a game, stream or AI model starts, Avix switches your PC into its tuned state and restores it the moment it closes.")).wrap());
                ui.add_space(10.0);
                let first = app.cfg.profiles.iter().find(|p| p.enabled).map(|p| p.name.clone());
                if Btn::new(Kind::Primary, "Boost now").icon(icon::ZAP).show(ui).clicked() {
                    app.monitor.send(Request::SetMode { mode: Mode::On, profile: first });
                }
                if hotkey {
                    ui.add_space(4.0);
                    ui.label(small("Or press Ctrl+Alt+B from anywhere."));
                }
                let enabled = app.cfg.profiles.iter().filter(|p| p.enabled).count();
                let triggers: usize = app.cfg.profiles.iter().filter(|p| p.enabled).map(|p| p.triggers.len()).sum();
                ui.add_space(10.0);
                divider(ui);
                ui.add_space(6.0);
                ui.horizontal(|ui| {
                    ui.label(small(format!("{enabled} profile{} · {triggers} apps watched", if enabled == 1 { "" } else { "s" })));
                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        if Btn::new(Kind::Ghost, "Edit").icon(icon::ARROW_RIGHT).small().show(ui).clicked() {
                            app.go(Page::Profiles);
                        }
                    });
                });
            }),
        },
    }
}

fn activity(ui: &mut Ui, s: &Snap) {
    card(ui, |ui| {
        section(ui, "Activity");
        ui.add_space(6.0);
        match &s.engine {
            None => empty_state(ui, icon::ACTIVITY, "No engine, no activity", "Start the engine to see what Avix is doing."),
            Some(e) if e.events.is_empty() => empty_state(ui, icon::ACTIVITY, "Nothing yet", "Boosts, purges and restores show up here."),
            Some(e) => {
                {
                    // newest first; a short fixed list instead of a nested scroll area
                    for ev in e.events.iter().rev().take(8) {
                        let tone = match ev.kind {
                            EventKind::Boost => Tone::Good,
                            EventKind::Warn => Tone::Warn,
                            EventKind::Info => Tone::Neutral,
                        };
                        ui.horizontal_top(|ui| {
                            let (r, _) = ui.allocate_exact_size(vec2(10.0, 18.0), Sense::hover());
                            ui.painter().circle_filled(pos2(r.center().x, r.top() + 9.0), 3.5, tone.solid());
                            ui.vertical(|ui| {
                                ui.spacing_mut().item_spacing.y = 1.0;
                                ui.add(egui::Label::new(tw(&ev.text, 13.0, if tone == Tone::Good { W::Medium } else { W::Regular })).wrap());
                                ui.label(time_t(&ev.time));
                            });
                        });
                        ui.add_space(6.0);
                    }
                }
            }
        }
    });
}
