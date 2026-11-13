use super::data::lock;
use super::theme::*;
use super::{App, Page, ToastKind};
use crate::ipc::{EventKind, Mode, Request};
use crate::sys::fmt_bytes;
use eframe::egui::{self, Align, Layout, RichText, Ui};

fn fmt_dur(secs: u64) -> String {
    if secs >= 3600 {
        format!("{}h {:02}m", secs / 3600, secs / 60 % 60)
    } else if secs >= 60 {
        format!("{}m {:02}s", secs / 60, secs % 60)
    } else {
        format!("{secs}s")
    }
}

pub fn show(app: &mut App, ui: &mut Ui) {
    page_header(ui, "Dashboard", "Live status of the Avix engine and your PC");

    let (engine, cpu, cpu_hist, mem, gpu, gpu_hist, vram, top, ping, ping_hist, stats, target) = {
        let m = lock(&app.monitor.metrics);
        (
            m.engine.clone(),
            m.cpu,
            m.cpu_hist.iter().copied().collect::<Vec<_>>(),
            m.mem,
            m.gpu,
            m.gpu_hist.iter().copied().collect::<Vec<_>>(),
            m.vram_used,
            m.procs.iter().filter(|p| p.name != "System" && p.name != "Memory Compression").take(8).cloned().collect::<Vec<_>>(),
            m.ping_ms,
            m.ping_hist.iter().map(|p| p.map(|v| v as f32).unwrap_or(0.0)).collect::<Vec<_>>(),
            m.ping_stats(),
            m.ping_target.clone(),
        )
    };

    // ---------------------------------------------------------------- hero
    let (label, color, sub) = match &engine {
        None => ("ENGINE OFFLINE", BAD, "The background engine isn't running, so nothing is being optimized.".to_string()),
        Some(s) => match (&s.active, s.mode) {
            (Some(a), _) => {
                let target = if a.target.is_empty() { "whole system".to_string() } else { a.target.clone() };
                ("BOOSTING", GOOD, format!("{}  -  {}  -  {}", a.profile, target, fmt_dur(a.since_secs)))
            }
            (None, Mode::Off) => ("PAUSED", MUTED, "Avix won't boost anything until you resume auto-boost.".into()),
            (None, _) => ("WATCHING", ACCENT, "Waiting for a game, stream or AI workload to start...".into()),
        },
    };
    let hero_color = if engine.as_ref().is_some_and(|s| s.active.is_some()) { GOOD } else { ACCENT };
    card_tinted(ui, hero_color, |ui| {
        ui.horizontal(|ui| {
            ui.vertical(|ui| {
                ui.horizontal(|ui| {
                    status_dot(ui, color, 7.0);
                    ui.label(RichText::new(label).size(26.0).strong().color(color));
                });
                ui.label(RichText::new(sub).size(15.0));
                if let Some(a) = engine.as_ref().and_then(|s| s.active.as_ref()) {
                    ui.label(small(&a.reason));
                    ui.add_space(4.0);
                    ui.horizontal_wrapped(|ui| {
                        if !a.power_plan.is_empty() {
                            chip(ui, format!("Power: {}", a.power_plan), INFO);
                        }
                        let s = |n: u32| if n == 1 { "" } else { "s" };
                        chip(ui, format!("{} app{} throttled", a.throttled, s(a.throttled)), ACCENT);
                        if a.raised > 0 {
                            chip(ui, format!("{} app{} prioritised", a.raised, s(a.raised)), GOOD);
                        }
                        if !a.services_paused.is_empty() {
                            chip(ui, format!("Paused: {}", a.services_paused.join(", ")), WARN);
                        }
                        if a.standby_purges > 0 {
                            chip(ui, format!("{} memory purge{}", a.standby_purges, s(a.standby_purges)), INFO);
                        }
                        if a.apps_closed > 0 {
                            chip(ui, format!("{} app{} closed", a.apps_closed, s(a.apps_closed)), WARN);
                        }
                        if a.timer_resolution {
                            chip(ui, "0.5 ms timer", INFO);
                        }
                        if a.protected_target {
                            chip(ui, "Anti-cheat safe: game process untouched", GOOD);
                        }
                    });
                }
            });
            ui.with_layout(Layout::right_to_left(Align::Min), |ui| match &engine {
                None => {
                    if primary(ui, "Start engine").clicked() {
                        app.start_engine();
                    }
                }
                Some(s) => {
                    ui.vertical(|ui| {
                        ui.with_layout(Layout::right_to_left(Align::Min), |ui| {
                            if ui.selectable_label(s.mode == Mode::Off, "  Pause  ").clicked() {
                                app.monitor.send(Request::SetMode { mode: Mode::Off, profile: None });
                            }
                            let names: Vec<String> = app.cfg.profiles.iter().filter(|p| p.enabled).map(|p| p.name.clone()).collect();
                            let boost_label = if s.mode == Mode::On { "  Boosting now  " } else { "  Boost now  " };
                            ui.menu_button(RichText::new(boost_label).color(if s.mode == Mode::On { GOOD } else { TEXT }), |ui| {
                                for n in names {
                                    if ui.button(&n).clicked() {
                                        app.monitor.send(Request::SetMode { mode: Mode::On, profile: Some(n) });
                                        ui.close_menu();
                                    }
                                }
                            });
                            if ui.selectable_label(s.mode == Mode::Auto, "  Auto  ").clicked() {
                                app.monitor.send(Request::SetMode { mode: Mode::Auto, profile: None });
                            }
                        });
                        ui.add_space(6.0);
                        ui.label(small(format!(
                            "Engine: {} RAM - {:.2}% CPU - up {}",
                            fmt_bytes(s.engine_ram),
                            s.engine_cpu_percent,
                            fmt_dur(s.uptime_secs)
                        )));
                        ui.label(small("Ctrl+Alt+B toggles Boost now"));
                    });
                }
            });
        });
    });
    ui.add_space(12.0);

    // ---------------------------------------------------------------- metrics
    ui.columns(4, |cols| {
        card(&mut cols[0], |ui| {
            section(ui, "CPU");
            ui.label(RichText::new(format!("{cpu:.0}%")).size(28.0).strong().color(severity_color(cpu)));
            ui.label(small(format!("{} logical cores", crate::sys::process::logical_cpus())));
            sparkline(ui, &cpu_hist, 100.0, ACCENT, 44.0);
        });
        card(&mut cols[1], |ui| {
            section(ui, "Memory");
            let used = mem.total.saturating_sub(mem.avail);
            ui.label(RichText::new(format!("{:.1} / {:.1} GB", used as f64 / 1073741824.0, mem.total as f64 / 1073741824.0)).size(24.0).strong());
            bar(ui, mem.load as f32 / 100.0, severity_color(mem.load as f32));
            match engine.as_ref().and_then(|s| s.standby.zip(s.free)) {
                Some((standby, free)) => {
                    ui.label(small(format!("Standby {}  -  Free {}", fmt_bytes(standby), fmt_bytes(free))));
                }
                None => {
                    ui.label(small(format!("{}% in use", mem.load)));
                }
            }
            if ui.small_button("Free standby memory").on_hover_text("Empties the standby cache (safe; Windows re-reads from disk if needed)").clicked() {
                if engine.as_ref().is_some_and(|s| s.elevated) {
                    app.monitor.send(Request::PurgeStandby);
                    app.toast(ToastKind::Info, "Purging standby memory...");
                } else {
                    app.toast(ToastKind::Err, "Needs the engine running as admin");
                }
            }
        });
        card(&mut cols[2], |ui| {
            section(ui, "GPU");
            match gpu {
                Some(g) => {
                    ui.label(RichText::new(format!("{g:.0}%")).size(28.0).strong().color(severity_color(g)));
                    ui.label(small(format!("VRAM in use: {}", fmt_bytes(vram))));
                    sparkline(ui, &gpu_hist, 100.0, GOOD, 44.0);
                }
                None => {
                    ui.label(RichText::new("n/a").size(28.0).strong().color(MUTED));
                    ui.label(small("GPU counters unavailable"));
                }
            }
        });
        card(&mut cols[3], |ui| {
            section(ui, "Network latency");
            let (txt, c) = match ping {
                Some(p) => (format!("{p} ms"), if p < 40 { GOOD } else if p < 90 { WARN } else { BAD }),
                None if stats.samples > 0 => ("timeout".into(), BAD),
                None => ("...".into(), MUTED),
            };
            ui.label(RichText::new(txt).size(28.0).strong().color(c));
            ui.label(small(format!(
                "jitter {:.0} ms  -  loss {:.0}%  -  {}",
                stats.jitter,
                stats.loss,
                if target.is_empty() { "-".to_string() } else { target.clone() }
            )));
            sparkline(ui, &ping_hist, 60.0, INFO, 44.0);
        });
    });
    ui.add_space(12.0);

    // ---------------------------------------------------------------- detected games
    if let Some(s) = &engine {
        let profile_names: Vec<String> = app.cfg.profiles.iter().map(|p| p.name.clone()).collect();
        let undetected: Vec<String> = s
            .detected_games
            .iter()
            .filter(|g| !app.cfg.profiles.iter().any(|p| p.triggers.iter().any(|t| t.eq_ignore_ascii_case(g))))
            .cloned()
            .collect();
        if !undetected.is_empty() {
            card_tinted(ui, INFO, |ui| {
                ui.label(RichText::new("New fullscreen apps detected").strong());
                ui.label(muted("Avix boosted these because they went fullscreen. Add them to a profile so they boost the moment they launch."));
                for g in undetected {
                    ui.horizontal(|ui| {
                        ui.label(RichText::new(&g).monospace());
                        for (i, n) in profile_names.iter().enumerate() {
                            if ui.small_button(format!("Add to {n}")).clicked() {
                                app.cfg.profiles[i].triggers.push(g.to_lowercase());
                                app.save_config();
                            }
                        }
                    });
                }
            });
            ui.add_space(12.0);
        }
    }

    // ---------------------------------------------------------------- processes + activity
    ui.columns(2, |cols| {
        card(&mut cols[0], |ui| {
            ui.horizontal(|ui| {
                section(ui, "Top processes");
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    if ui.small_button("See all").clicked() {
                        app.page = Page::Processes;
                    }
                });
            });
            let name_w = (ui.available_width() - 170.0).max(120.0);
            egui::Grid::new("top-procs").num_columns(3).spacing([16.0, 6.0]).striped(false).show(ui, |ui| {
                cell(ui, name_w, 16.0, small("Name"));
                ui.label(small("CPU"));
                ui.label(small("Memory"));
                ui.end_row();
                for p in &top {
                    cell(ui, name_w, 18.0, &p.name);
                    ui.label(RichText::new(format!("{:.1}%", p.cpu)).color(if p.cpu > 10.0 { WARN } else { TEXT }));
                    ui.label(fmt_bytes(p.mem));
                    ui.end_row();
                }
            });
        });
        card(&mut cols[1], |ui| {
            section(ui, "Activity");
            match &engine {
                None => {
                    ui.label(muted("Start the engine to see what Avix is doing."));
                }
                Some(s) if s.events.is_empty() => {
                    ui.label(muted("Nothing yet."));
                }
                Some(s) => {
                    egui::ScrollArea::vertical().id_salt("events").max_height(250.0).auto_shrink([false, true]).show(ui, |ui| {
                        for e in s.events.iter().rev().take(40) {
                            let c = match e.kind {
                                EventKind::Boost => GOOD,
                                EventKind::Warn => WARN,
                                EventKind::Info => MUTED,
                            };
                            ui.horizontal_top(|ui| {
                                ui.label(RichText::new(&e.time).size(12.0).color(FAINT).monospace());
                                ui.add(egui::Label::new(RichText::new(&e.text).size(13.0).color(if c == MUTED { TEXT } else { c })).wrap());
                            });
                        }
                    });
                }
            }
        });
    });
}
