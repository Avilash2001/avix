use super::data::lock;
use super::theme::*;
use super::{App, ToastKind};
use crate::config::{self, PowerPlan, Profile, TargetPriority};
use crate::tweaks;
use eframe::egui::{self, Align, Key, Layout, RichText, Ui};
use std::collections::HashMap;

#[derive(Default)]
pub struct State {
    pub selected: usize,
    new_trigger: String,
    new_companion: String,
    new_service: String,
    new_close: String,
    ifeo: HashMap<String, Option<u32>>,
}

/// Chip list + add box + optional suggestions. Returns nothing; edits in place.
pub fn list_editor(
    ui: &mut Ui,
    id: &str,
    list: &mut Vec<String>,
    buf: &mut String,
    hint: &str,
    suggestions: &[(String, String)],
    lowercase: bool,
) {
    ui.horizontal_wrapped(|ui| {
        let mut remove = None;
        for (i, item) in list.iter().enumerate() {
            if removable_chip(ui, item) {
                remove = Some(i);
            }
        }
        if let Some(i) = remove {
            list.remove(i);
        }
        if list.is_empty() {
            ui.label(small("None"));
        }
    });
    ui.horizontal(|ui| {
        let te = ui.add(egui::TextEdit::singleline(buf).hint_text(hint).desired_width(220.0));
        let enter = te.lost_focus() && ui.input(|i| i.key_pressed(Key::Enter));
        if (ui.button("Add").clicked() || enter) && !buf.trim().is_empty() {
            let v = if lowercase { buf.trim().to_lowercase() } else { buf.trim().to_string() };
            if !list.iter().any(|x| x.eq_ignore_ascii_case(&v)) {
                list.push(v);
            }
            buf.clear();
        }
        let avail: Vec<&(String, String)> =
            suggestions.iter().filter(|(v, _)| !list.iter().any(|x| x.eq_ignore_ascii_case(v))).collect();
        if !avail.is_empty() {
            egui::ComboBox::from_id_salt(id).selected_text(small("Suggestions")).width(240.0).height(320.0).show_ui(ui, |ui| {
                for (v, label) in avail {
                    if ui.selectable_label(false, label).clicked() {
                        list.push(v.clone());
                    }
                }
            });
        }
    });
}

pub fn show(app: &mut App, ui: &mut Ui) {
    page_header(
        ui,
        "Boost profiles",
        "Avix boosts automatically while one of a profile's apps is running. The first matching profile (top of the list) wins.",
    );

    let running: Vec<(String, String)> = {
        let m = lock(&app.monitor.metrics);
        let mut names: Vec<String> = m.procs.iter().filter(|p| p.user_session).map(|p| p.name_lc.clone()).collect();
        names.sort();
        names.dedup();
        names.into_iter().map(|n| (n.clone(), n)).collect()
    };

    if app.cfg.profiles.is_empty() {
        app.cfg.profiles.push(Profile::default());
    }
    let elevated = app.elevated;
    let mut delete: Option<usize> = None;
    let mut toasts: Vec<(ToastKind, String)> = Vec::new();

    ui.horizontal_top(|ui| {
        // ------------------------------------------------------------ list
        ui.vertical(|ui| {
            ui.set_width(240.0);
            card(ui, |ui| {
                section(ui, "Profiles (priority order)");
                let n = app.cfg.profiles.len();
                ui.with_layout(Layout::top_down_justified(Align::LEFT), |ui| {
                    for i in 0..n {
                        let p = &app.cfg.profiles[i];
                        let selected = app.profiles.selected == i;
                        let text = if p.enabled {
                            RichText::new(format!("{}.  {}", i + 1, p.name)).size(14.5)
                        } else {
                            RichText::new(format!("{}.  {}  (off)", i + 1, p.name)).size(14.5).color(FAINT)
                        };
                        if ui.selectable_label(selected, text).clicked() {
                            app.profiles.selected = i;
                        }
                    }
                });
                ui.add_space(6.0);
                ui.horizontal(|ui| {
                    if ui.button("+ New").clicked() {
                        app.cfg.profiles.push(Profile::default());
                        app.profiles.selected = app.cfg.profiles.len() - 1;
                    }
                    let s = app.profiles.selected;
                    if ui.add_enabled(s > 0, egui::Button::new("Up")).clicked() {
                        app.cfg.profiles.swap(s, s - 1);
                        app.profiles.selected -= 1;
                    }
                    if ui.add_enabled(s + 1 < n, egui::Button::new("Down")).clicked() {
                        app.cfg.profiles.swap(s, s + 1);
                        app.profiles.selected += 1;
                    }
                });
            });
            ui.add_space(8.0);
            card(ui, |ui| {
                section(ui, "How it works");
                ui.label(small("While boosting, Avix switches to a tuned power plan, raises your app, throttles everything else in your session (EcoQoS + lower CPU/IO/memory priority), pauses update services and keeps all of it applied to newly started apps. When the app exits, everything is restored exactly."));
            });
        });

        // ------------------------------------------------------------ editor
        ui.vertical(|ui| {
            let idx = app.profiles.selected.min(app.cfg.profiles.len() - 1);
            app.profiles.selected = idx;
            let can_delete = app.cfg.profiles.len() > 1;
            let protected_list = app.cfg.anticheat_protected.clone();
            let st = &mut app.profiles;
            let p = &mut app.cfg.profiles[idx];

            card(ui, |ui| {
                ui.horizontal(|ui| {
                    ui.add(egui::TextEdit::singleline(&mut p.name).font(egui::TextStyle::Heading).desired_width(260.0));
                    ui.add_space(8.0);
                    toggle(ui, &mut p.enabled);
                    ui.label(if p.enabled { "Enabled" } else { "Disabled" });
                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        if can_delete && danger(ui, "Delete").clicked() {
                            delete = Some(idx);
                        }
                    });
                });
            });
            ui.add_space(8.0);

            card(ui, |ui| {
                section(ui, "Activates when any of these apps run");
                if p.triggers.is_empty() {
                    ui.label(muted("No apps yet - add the game or app's .exe name."));
                }
                let mut remove = None;
                egui::Grid::new("triggers").num_columns(3).spacing([14.0, 4.0]).show(ui, |ui| {
                    for (i, t) in p.triggers.iter().enumerate() {
                        ui.horizontal(|ui| {
                            ui.label(RichText::new(t).monospace());
                            if config::contains(&protected_list, t) {
                                chip(ui, "anti-cheat", WARN).on_hover_text(
                                    "Avix never opens this game's process. It optimizes everything around it instead. Use 'Launch priority' to raise the game itself safely.",
                                );
                            }
                        });
                        let cur = *st.ifeo.entry(t.clone()).or_insert_with(|| tweaks::launch_priority(t));
                        let label = match cur {
                            Some(3) => "Launch priority: High",
                            Some(6) => "Launch priority: Above normal",
                            Some(_) => "Launch priority: custom",
                            None => "Launch priority: default",
                        };
                        ui.add_enabled_ui(elevated, |ui| {
                            egui::ComboBox::from_id_salt(("ifeo", i)).selected_text(small(label)).width(210.0).show_ui(ui, |ui| {
                                for (v, l) in [(None, "Default (Windows decides)"), (Some(6u32), "Above normal"), (Some(3u32), "High")] {
                                    if ui.selectable_label(cur == v, l).clicked() {
                                        match tweaks::set_launch_priority(t, v) {
                                            Ok(()) => {
                                                st.ifeo.insert(t.clone(), v);
                                                toasts.push((ToastKind::Ok, format!("{t} will start with {l} priority")));
                                            }
                                            Err(e) => toasts.push((ToastKind::Err, e)),
                                        }
                                    }
                                }
                            });
                        })
                        .response
                        .on_hover_text("Windows applies this when the app starts (Image File Execution Options). Takes effect next launch.");
                        if ui.small_button("Remove").clicked() {
                            remove = Some(i);
                        }
                        ui.end_row();
                    }
                });
                if let Some(i) = remove {
                    p.triggers.remove(i);
                }
                ui.add_space(4.0);
                ui.horizontal(|ui| {
                    let te = ui.add(egui::TextEdit::singleline(&mut st.new_trigger).hint_text("game.exe").desired_width(220.0));
                    let enter = te.lost_focus() && ui.input(|i| i.key_pressed(Key::Enter));
                    if (ui.button("Add").clicked() || enter) && !st.new_trigger.trim().is_empty() {
                        let v = st.new_trigger.trim().to_lowercase();
                        if !p.triggers.contains(&v) {
                            p.triggers.push(v);
                        }
                        st.new_trigger.clear();
                    }
                    egui::ComboBox::from_id_salt("pick-running").selected_text(small("Pick a running app")).width(260.0).height(360.0).show_ui(ui, |ui| {
                        for (n, _) in &running {
                            if !p.triggers.contains(n) && ui.selectable_label(false, n).clicked() {
                                p.triggers.push(n.clone());
                            }
                        }
                    });
                });
            });
            ui.add_space(8.0);

            card(ui, |ui| {
                section(ui, "While boosting");
                egui::Grid::new("boost-opts").num_columns(2).spacing([18.0, 10.0]).show(ui, |ui| {
                    ui.label("Power plan");
                    egui::ComboBox::from_id_salt("plan").selected_text(p.power_plan.label()).width(240.0).show_ui(ui, |ui| {
                        for v in PowerPlan::ALL {
                            ui.selectable_value(&mut p.power_plan, v, v.label());
                        }
                    });
                    ui.end_row();
                    ui.label("App priority");
                    egui::ComboBox::from_id_salt("prio").selected_text(p.priority.label()).width(240.0).show_ui(ui, |ui| {
                        for v in TargetPriority::ALL {
                            ui.selectable_value(&mut p.priority, v, v.label());
                        }
                    });
                    ui.end_row();
                    ui.label("Only while busy");
                    ui.horizontal(|ui| {
                        ui.add(egui::Slider::new(&mut p.min_cpu_core_percent, 0.0..=100.0).step_by(5.0).suffix("% of a core"));
                        ui.label(small(if p.min_cpu_core_percent > 0.0 { "boosts only while the app is working" } else { "0 = whenever it runs" }));
                    });
                    ui.end_row();
                });
                ui.add_space(6.0);
                toggle_row(ui, &mut p.throttle_background, "Throttle background apps", "Lower CPU & memory priority for everything else in your session (the window you're looking at is always exempt).");
                toggle_row(ui, &mut p.efficiency_mode, "Efficiency mode (EcoQoS) for throttled apps", "Same as Task Manager's Efficiency mode - keeps background work off your game's cores and clocks.");
                toggle_row(ui, &mut p.lower_io_priority, "Low disk priority for throttled apps", "Background downloads, sync and indexing yield disk access to your game (less asset-streaming stutter).");
                toggle_row(ui, &mut p.standby_cleaner, "Standby memory cleaner", "Empties the standby list when free RAM runs low - fixes stutter on RAM-tight systems (like ISLC).");
                toggle_row(ui, &mut p.timer_resolution, "0.5 ms timer resolution", "Requests the finest system timer. On Windows 11 it only reaches games with the 'global timer requests' tweak.");
            });
            ui.add_space(8.0);

            let services: Vec<(String, String)> =
                config::SUGGESTED_SERVICES.iter().map(|(n, l)| (n.to_string(), format!("{l} ({n})"))).collect();
            let close: Vec<(String, String)> =
                config::SUGGESTED_CLOSE_APPS.iter().map(|n| (n.to_string(), n.to_string())).collect();
            card(ui, |ui| {
                section(ui, "Keep these responsive (companions)");
                ui.label(small("Raised to Above normal and never throttled - e.g. OBS while streaming a game."));
                list_editor(ui, "companions", &mut p.companions, &mut st.new_companion, "obs64.exe", &running, true);
                ui.add_space(8.0);
                section(ui, "Pause these services (restarted afterwards)");
                ui.label(small("Stops Windows Update & friends from downloading/installing mid-match. Needs admin."));
                list_editor(ui, "services", &mut p.pause_services, &mut st.new_service, "service name", &services, false);
                ui.add_space(8.0);
                section(ui, "Close these apps when the boost starts");
                ui.label(small("They are terminated (not restarted) - only list apps that never hold unsaved work."));
                list_editor(ui, "close", &mut p.close_apps, &mut st.new_close, "app.exe", &close, true);
            });
        });
    });

    for (k, t) in toasts {
        app.toast(k, t);
    }
    if let Some(i) = delete {
        let name = app.cfg.profiles[i].name.clone();
        app.ask("Delete profile?", &format!("Delete the '{name}' profile? (Not saved until you press Save.)"), "Delete", true, move |app| {
            if i < app.cfg.profiles.len() {
                app.cfg.profiles.remove(i);
                app.profiles.selected = 0;
            }
        });
    }
}
