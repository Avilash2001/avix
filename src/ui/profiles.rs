use super::dashboard::app_avatar;
use super::data::lock;
use super::theme::*;
use super::{App, ToastKind};
use crate::config::{self, PowerPlan, Profile, TargetPriority};
use crate::tweaks;
use eframe::egui::{self, pos2, vec2, Align, Align2, CornerRadius, Frame, Key, Layout, Margin, Sense, Ui};
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

/// Chip list + add box + optional suggestions. Edits in place.
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
        ui.spacing_mut().item_spacing = vec2(6.0, 6.0);
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
            ui.label(small("None yet"));
        }
    });
    ui.add_space(4.0);
    ui.horizontal_wrapped(|ui| {
        let te = text_field(ui, (id, "add"), buf, hint, 220.0, None);
        let enter = te.lost_focus() && ui.input(|i| i.key_pressed(Key::Enter));
        if (Btn::new(Kind::Secondary, "Add").icon(icon::PLUS).show(ui).clicked() || enter) && !buf.trim().is_empty() {
            let v = if lowercase { buf.trim().to_lowercase() } else { buf.trim().to_string() };
            if !list.iter().any(|x| x.eq_ignore_ascii_case(&v)) {
                list.push(v);
            }
            buf.clear();
        }
        let avail: Vec<&(String, String)> = suggestions.iter().filter(|(v, _)| !list.iter().any(|x| x.eq_ignore_ascii_case(v))).collect();
        if !avail.is_empty() {
            combo(id).selected_text(muted_t("Suggestions")).width(220.0).height(320.0).show_ui(ui, |ui| {
                for (v, label) in avail {
                    if ui.selectable_label(false, label).clicked() {
                        list.push(v.clone());
                    }
                }
            });
        }
    });
}

fn profile_row(ui: &mut Ui, i: usize, p: &Profile, selected: bool) -> egui::Response {
    let (rect, resp) = ui.allocate_exact_size(vec2(ui.available_width(), 46.0), Sense::click());
    let pl = pal();
    let hov = ease_bool(ui.ctx(), resp.id.with("h"), resp.hovered(), 0.12);
    let fill = if selected { pl.sunken } else { mix(egui::Color32::TRANSPARENT, pl.sunken, hov) };
    ui.painter().rect_filled(rect, CornerRadius::same(12), fill);
    let c = pos2(rect.left() + 22.0, rect.center().y);
    ui.painter().circle_filled(c, 13.0, if selected { pl.accent } else { pl.hover });
    ui.painter().text(c, Align2::CENTER_CENTER, (i + 1).to_string(), font(12.0, W::Semi), if selected { egui::Color32::WHITE } else { pl.muted });
    let name_color = if p.enabled { pl.text } else { pl.faint };
    ui.painter().text(pos2(rect.left() + 46.0, rect.center().y - 8.0), Align2::LEFT_CENTER, &p.name, font(14.0, W::Medium), name_color);
    let sub = if !p.enabled {
        "Off".to_string()
    } else {
        format!("{} app{}", p.triggers.len(), if p.triggers.len() == 1 { "" } else { "s" })
    };
    ui.painter().text(pos2(rect.left() + 46.0, rect.center().y + 9.0), Align2::LEFT_CENTER, sub, font(11.5, W::Regular), pl.faint);
    resp.on_hover_cursor(egui::CursorIcon::PointingHand)
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
    let mut delete: Option<usize> = None;
    let mut toasts: Vec<(ToastKind, String)> = Vec::new();

    let w = ui.available_width();
    if w >= 900.0 {
        let list_w = 270.0;
        ui.horizontal_top(|ui| {
            ui.spacing_mut().item_spacing.x = 18.0;
            ui.allocate_ui_with_layout(vec2(list_w, 0.0), Layout::top_down(Align::Min), |ui| {
                ui.set_width(list_w);
                list_column(app, ui);
            });
            let ew = ui.available_width();
            ui.allocate_ui_with_layout(vec2(ew, 0.0), Layout::top_down(Align::Min), |ui| {
                ui.set_width(ew);
                editor(app, ui, &running, &mut delete, &mut toasts);
            });
        });
    } else {
        list_column(app, ui);
        ui.add_space(14.0);
        editor(app, ui, &running, &mut delete, &mut toasts);
    }

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

fn list_column(app: &mut App, ui: &mut Ui) {
    card(ui, |ui| {
        section(ui, "Profiles");
        ui.label(small("In priority order"));
        ui.add_space(8.0);
        ui.spacing_mut().item_spacing.y = 4.0;
        let n = app.cfg.profiles.len();
        for i in 0..n {
            if profile_row(ui, i, &app.cfg.profiles[i], app.profiles.selected == i).clicked() {
                app.profiles.selected = i;
            }
        }
        ui.add_space(8.0);
        ui.horizontal(|ui| {
            if Btn::new(Kind::Secondary, "New").icon(icon::PLUS).small().show(ui).clicked() {
                app.cfg.profiles.push(Profile::default());
                app.profiles.selected = app.cfg.profiles.len() - 1;
            }
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                let s = app.profiles.selected;
                if Btn::new(Kind::Ghost, "").icon(icon::ARROW_DOWN).small().enabled(s + 1 < n).show(ui).on_hover_text("Move down").clicked() {
                    app.cfg.profiles.swap(s, s + 1);
                    app.profiles.selected += 1;
                }
                if Btn::new(Kind::Ghost, "").icon(icon::ARROW_UP).small().enabled(s > 0).show(ui).on_hover_text("Move up").clicked() {
                    app.cfg.profiles.swap(s, s - 1);
                    app.profiles.selected -= 1;
                }
            });
        });
    });
    ui.add_space(14.0);
    card(ui, |ui| {
        ui.horizontal(|ui| {
            icon_label(ui, icon::INFO, 15.0, muted());
            ui.label(strong("How a boost works"));
        });
        ui.add(egui::Label::new(small("Avix switches to a tuned power plan, raises your app, throttles everything else in your session (EcoQoS + lower CPU/IO/memory priority), pauses update services and keeps all of it applied to newly started apps. When the app exits, everything is restored exactly.")).wrap());
    });
}

fn editor(app: &mut App, ui: &mut Ui, running: &[(String, String)], delete: &mut Option<usize>, toasts: &mut Vec<(ToastKind, String)>) {
    let idx = app.profiles.selected.min(app.cfg.profiles.len() - 1);
    app.profiles.selected = idx;
    let can_delete = app.cfg.profiles.len() > 1;
    let protected_list = app.cfg.anticheat_protected.clone();
    let elevated = app.elevated;
    let st = &mut app.profiles;
    let p = &mut app.cfg.profiles[idx];

    card(ui, |ui| {
        split_row(
            ui,
            "profile-name",
            if can_delete { 250.0 } else { 150.0 },
            |ui| {
                ui.label(label_t("Profile name"));
                Frame::new().fill(sunken()).corner_radius(CornerRadius::same(10)).inner_margin(Margin::symmetric(12, 6)).show(ui, |ui| {
                    ui.add(egui::TextEdit::singleline(&mut p.name).frame(false).font(font(18.0, W::Semi)).desired_width(ui.available_width().min(360.0)).margin(Margin::ZERO));
                });
            },
            |ui| {
                if can_delete && Btn::new(Kind::Danger, "Delete").icon(icon::TRASH).small().show(ui).clicked() {
                    *delete = Some(idx);
                }
                ui.add_space(10.0);
                toggle(ui, &mut p.enabled);
                ui.label(t(if p.enabled { "Enabled" } else { "Disabled" }));
            },
        );
    });
    ui.add_space(14.0);

    card(ui, |ui| {
        section(ui, "Activates when any of these apps run");
        ui.add_space(4.0);
        if p.triggers.is_empty() {
            ui.label(muted_t("No apps yet — add the game or app's .exe name, or pick one that's running."));
        }
        let mut remove = None;
        for (i, tr) in p.triggers.iter().enumerate() {
            ui.add_space(6.0);
            divider(ui);
            ui.add_space(6.0);
            split_row(
                ui,
                ("trigger", i),
                300.0,
                |ui| {
                    ui.horizontal(|ui| {
                        app_avatar(ui, tr, 26.0);
                        ui.add(egui::Label::new(t(tr)).truncate());
                        if config::contains(&protected_list, tr) {
                            chip_icon(ui, Some(icon::SHIELD_CHECK), "Anti-cheat", Tone::Neutral).on_hover_text(
                                "Avix never opens this game's process. It optimizes everything around it instead. Use 'Launch priority' to raise the game itself safely.",
                            );
                        }
                    });
                },
                |ui| {
                    if Btn::new(Kind::Ghost, "").icon(icon::X).small().show(ui).on_hover_text("Remove").clicked() {
                        remove = Some(i);
                    }
                    let cur = *st.ifeo.entry(tr.clone()).or_insert_with(|| tweaks::launch_priority(tr));
                    let label = match cur {
                        Some(3) => "Launch priority: High",
                        Some(6) => "Launch priority: Above normal",
                        Some(_) => "Launch priority: custom",
                        None => "Launch priority: default",
                    };
                    ui.add_enabled_ui(elevated, |ui| {
                        combo(("ifeo", i)).selected_text(small(label)).width(220.0).show_ui(ui, |ui| {
                            for (v, l) in [(None, "Default (Windows decides)"), (Some(6u32), "Above normal"), (Some(3u32), "High")] {
                                if ui.selectable_label(cur == v, l).clicked() {
                                    match tweaks::set_launch_priority(tr, v) {
                                        Ok(()) => {
                                            st.ifeo.insert(tr.clone(), v);
                                            toasts.push((ToastKind::Ok, format!("{tr} will start with {l} priority")));
                                        }
                                        Err(e) => toasts.push((ToastKind::Err, e)),
                                    }
                                }
                            }
                        });
                    })
                    .response
                    .on_hover_text("Windows applies this when the app starts (Image File Execution Options). Takes effect next launch.");
                },
            );
        }
        if let Some(i) = remove {
            p.triggers.remove(i);
        }
        ui.add_space(10.0);
        ui.horizontal_wrapped(|ui| {
            let te = text_field(ui, "new-trigger", &mut st.new_trigger, "game.exe", 220.0, None);
            let enter = te.lost_focus() && ui.input(|i| i.key_pressed(Key::Enter));
            if (Btn::new(Kind::Secondary, "Add").icon(icon::PLUS).show(ui).clicked() || enter) && !st.new_trigger.trim().is_empty() {
                let v = st.new_trigger.trim().to_lowercase();
                if !p.triggers.contains(&v) {
                    p.triggers.push(v);
                }
                st.new_trigger.clear();
            }
            combo("pick-running").selected_text(muted_t("Pick a running app")).width(240.0).height(360.0).show_ui(ui, |ui| {
                for (n, _) in running {
                    if !p.triggers.contains(n) && ui.selectable_label(false, n).clicked() {
                        p.triggers.push(n.clone());
                    }
                }
            });
        });
    });
    ui.add_space(14.0);

    card(ui, |ui| {
        section(ui, "While boosting");
        ui.add_space(8.0);
        split_row(ui, "plan", 250.0, |ui| row_label(ui, "Power plan", "Avix Ultimate is a tuned copy of Windows' hidden Ultimate plan; your plan comes back after."), |ui| {
            combo("plan").selected_text(p.power_plan.label()).width(240.0).show_ui(ui, |ui| {
                for v in PowerPlan::ALL {
                    ui.selectable_value(&mut p.power_plan, v, v.label());
                }
            });
        });
        ui.add_space(10.0);
        split_row(ui, "prio", 250.0, |ui| row_label(ui, "App priority", "CPU priority for the app that triggered the boost."), |ui| {
            combo("prio").selected_text(p.priority.label()).width(240.0).show_ui(ui, |ui| {
                for v in TargetPriority::ALL {
                    ui.selectable_value(&mut p.priority, v, v.label());
                }
            });
        });
        ui.add_space(10.0);
        let busy_hint = if p.min_cpu_core_percent > 0.0 { "Boosts only while the app is actually working." } else { "0 = boost whenever it runs." };
        split_row(ui, "busy", 290.0, |ui| row_label(ui, "Only while busy", busy_hint), |ui| {
            let mut v = p.min_cpu_core_percent as f64;
            if slider(ui, "busy", &mut v, 0.0..=100.0, 5.0, &|v| format!("{v:.0}% core")) {
                p.min_cpu_core_percent = v as f32;
            }
        });
        ui.add_space(12.0);
        divider(ui);
        ui.add_space(12.0);
        toggle_row(ui, &mut p.throttle_background, "Throttle background apps", "Lower CPU & memory priority for everything else in your session (the window you're looking at is always exempt).");
        toggle_row(ui, &mut p.efficiency_mode, "Efficiency mode (EcoQoS) for throttled apps", "Same as Task Manager's Efficiency mode — keeps background work off your game's cores and clocks.");
        toggle_row(ui, &mut p.lower_io_priority, "Low disk priority for throttled apps", "Background downloads, sync and indexing yield disk access to your game (less asset-streaming stutter).");
        toggle_row(ui, &mut p.standby_cleaner, "Standby memory cleaner", "Empties the standby list when free RAM runs low — fixes stutter on RAM-tight systems (like ISLC).");
        toggle_row(ui, &mut p.trim_memory, "Free memory from background apps when RAM is low", "At 80% RAM use or less than 1.5 GB free, throttled apps hand their memory back (at most once a minute). A big help on 8 GB PCs.");
        toggle_row(ui, &mut p.timer_resolution, "0.5 ms timer resolution", "Requests the finest system timer. On Windows 11 it only reaches games with the 'global timer requests' tweak.");
    });
    ui.add_space(14.0);

    let services: Vec<(String, String)> = config::SUGGESTED_SERVICES.iter().map(|(n, l)| (n.to_string(), format!("{l} ({n})"))).collect();
    let close: Vec<(String, String)> = config::SUGGESTED_CLOSE_APPS.iter().map(|n| (n.to_string(), n.to_string())).collect();
    card(ui, |ui| {
        section(ui, "Keep these responsive");
        ui.add(egui::Label::new(small("Companions are raised to Above normal and never throttled — e.g. OBS while streaming a game.")).wrap());
        ui.add_space(6.0);
        list_editor(ui, "companions", &mut p.companions, &mut st.new_companion, "obs64.exe", running, true);
        ui.add_space(14.0);
        divider(ui);
        ui.add_space(14.0);
        section(ui, "Pause these services");
        ui.add(egui::Label::new(small("Stops Windows Update & friends from downloading or installing mid-match; restarted afterwards. Needs admin.")).wrap());
        ui.add_space(6.0);
        list_editor(ui, "services", &mut p.pause_services, &mut st.new_service, "service name", &services, false);
        ui.add_space(14.0);
        divider(ui);
        ui.add_space(14.0);
        section(ui, "Close these apps when the boost starts");
        ui.add(egui::Label::new(small("They are terminated (not restarted) — only list apps that never hold unsaved work.")).wrap());
        ui.add_space(6.0);
        list_editor(ui, "close", &mut p.close_apps, &mut st.new_close, "app.exe", &close, true);
    });
}
