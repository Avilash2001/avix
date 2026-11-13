use super::dashboard::app_avatar;
use super::data::{lock, ProcRow};
use super::theme::*;
use super::{App, ToastKind};
use crate::sys::{fmt_bytes, process};
use eframe::egui::{self, pos2, vec2, Align, CornerRadius, Layout, Margin, Sense, Ui};

#[derive(Clone, Copy, PartialEq, Eq, Default)]
pub enum Sort {
    #[default]
    Cpu,
    Gpu,
    Memory,
    Name,
}

pub struct State {
    pub filter: String,
    sort: Sort,
    only_mine: bool,
}

impl Default for State {
    fn default() -> Self {
        Self { filter: String::new(), sort: Sort::Cpu, only_mine: true }
    }
}

enum Act {
    Priority(u32, u32, String),
    Efficiency(u32, bool, String),
    AddTrigger(usize, String),
    NeverTouch(String),
    Kill(u32, String),
    OpenLocation(u32),
}

const W_PID: f32 = 64.0;
const W_CPU: f32 = 64.0;
const W_GPU: f32 = 56.0;
const W_MEM: f32 = 84.0;
const W_PRIO: f32 = 104.0;
const W_ACT: f32 = 36.0;
const ROW_H: f32 = 40.0;

fn header(ui: &mut Ui, label: &str, sort: Option<Sort>, cur: &mut Sort, w: f32, right: bool) {
    let active = sort.is_some_and(|s| s == *cur);
    let (rect, resp) = ui.allocate_exact_size(vec2(w, 24.0), if sort.is_some() { Sense::click() } else { Sense::hover() });
    let color = if active { pal().accent_text } else if resp.hovered() && sort.is_some() { text() } else { muted() };
    let g = ui.painter().layout_no_wrap(label.to_string(), font(12.0, W::Medium), color);
    let iw = if active { 14.0 } else { 0.0 };
    let x = if right { rect.right() - g.size().x - iw } else { rect.left() };
    let y = rect.center().y - g.size().y / 2.0;
    ui.painter().galley(pos2(x, y), g.clone(), color);
    if active {
        let dir = if *cur == Sort::Name { icon::ARROW_UP } else { icon::ARROW_DOWN };
        paint_icon(ui, pos2(x + g.size().x + 8.0, rect.center().y), dir, 12.0, color);
    }
    if let Some(s) = sort {
        if resp.on_hover_cursor(egui::CursorIcon::PointingHand).clicked() {
            *cur = s;
        }
    }
}

fn row_menu(ui: &mut Ui, r: &ProcRow, profile_names: &[String], act: &mut Option<Act>) {
    ui.set_min_width(210.0);
    ui.label(strong(&r.name));
    ui.label(time_t(format!("PID {}", r.pid)));
    ui.separator();
    ui.menu_button("Set priority", |ui| {
        for (class, label) in process::USER_PRIORITIES {
            if ui.button(label).clicked() {
                *act = Some(Act::Priority(r.pid, class, r.name.clone()));
                ui.close_menu();
            }
        }
    });
    if ui.button("Efficiency mode on").clicked() {
        *act = Some(Act::Efficiency(r.pid, true, r.name.clone()));
        ui.close_menu();
    }
    if ui.button("Efficiency mode off").clicked() {
        *act = Some(Act::Efficiency(r.pid, false, r.name.clone()));
        ui.close_menu();
    }
    ui.separator();
    ui.menu_button("Boost with profile", |ui| {
        for (pi, n) in profile_names.iter().enumerate() {
            if ui.button(n).clicked() {
                *act = Some(Act::AddTrigger(pi, r.name_lc.clone()));
                ui.close_menu();
            }
        }
    });
    if ui.button("Never touch this app").clicked() {
        *act = Some(Act::NeverTouch(r.name_lc.clone()));
        ui.close_menu();
    }
    if ui.button("Open file location").clicked() {
        *act = Some(Act::OpenLocation(r.pid));
        ui.close_menu();
    }
    ui.separator();
    if ui.button(t("End process").color(Tone::Bad.fg())).clicked() {
        *act = Some(Act::Kill(r.pid, r.name.clone()));
        ui.close_menu();
    }
}

pub fn show(app: &mut App, ui: &mut Ui) {
    page_header(ui, "Processes", "Everything running right now. Use the ··· button or right-click a row for actions.");

    let mut rows: Vec<ProcRow> = {
        let m = lock(&app.monitor.metrics);
        let st = &app.processes;
        let f = st.filter.trim().to_lowercase();
        m.procs
            .iter()
            .filter(|p| (!st.only_mine || p.user_session) && (f.is_empty() || p.name_lc.contains(&f) || p.pid.to_string() == f))
            .cloned()
            .collect()
    };
    let st = &mut app.processes;
    ui.horizontal(|ui| {
        let w = (ui.available_width() * 0.4).clamp(200.0, 320.0);
        text_field(ui, "proc-filter", &mut st.filter, "Search by name or PID", w, Some(icon::SEARCH));
        ui.add_space(10.0);
        toggle(ui, &mut st.only_mine);
        ui.label(t("Only my apps"));
        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
            ui.label(small(format!("{} process{}", rows.len(), if rows.len() == 1 { "" } else { "es" })));
        });
    });
    ui.add_space(12.0);

    match st.sort {
        Sort::Cpu => rows.sort_by(|a, b| b.cpu.total_cmp(&a.cpu).then(b.mem.cmp(&a.mem))),
        Sort::Gpu => rows.sort_by(|a, b| b.gpu.total_cmp(&a.gpu).then(b.cpu.total_cmp(&a.cpu))),
        Sort::Memory => rows.sort_by(|a, b| b.mem.cmp(&a.mem)),
        Sort::Name => rows.sort_by(|a, b| a.name_lc.cmp(&b.name_lc)),
    }

    let profile_names: Vec<String> = app.cfg.profiles.iter().map(|p| p.name.clone()).collect();
    let mut act: Option<Act> = None;

    card_frame().inner_margin(Margin { left: 10, right: 10, top: 10, bottom: 6 }).show(ui, |ui| {
        ui.set_width(ui.available_width());
        let w = ui.available_width();
        let show_pid = w > 860.0;
        let show_gpu = w > 720.0;
        let show_prio = w > 620.0;
        let fixed = W_CPU + W_MEM + W_ACT + 34.0 + if show_pid { W_PID } else { 0.0 } + if show_gpu { W_GPU } else { 0.0 } + if show_prio { W_PRIO } else { 0.0 };
        let gaps = 8.0 * (3 + show_pid as usize + show_gpu as usize + show_prio as usize) as f32;
        let name_w = (w - fixed - gaps - 20.0).max(120.0);
        ui.horizontal(|ui| {
            ui.add_space(46.0);
            header(ui, "Name", Some(Sort::Name), &mut st.sort, name_w, false);
            if show_pid {
                header(ui, "PID", None, &mut st.sort, W_PID, true);
            }
            header(ui, "CPU", Some(Sort::Cpu), &mut st.sort, W_CPU, true);
            if show_gpu {
                header(ui, "GPU", Some(Sort::Gpu), &mut st.sort, W_GPU, true);
            }
            header(ui, "Memory", Some(Sort::Memory), &mut st.sort, W_MEM, true);
            if show_prio {
                ui.add_space(10.0);
                header(ui, "Priority", None, &mut st.sort, W_PRIO - 10.0, false);
            }
        });
        ui.add_space(2.0);
        divider(ui);
        if rows.is_empty() {
            empty_state(ui, icon::SEARCH, "No processes match", "Try another name, or turn off \"Only my apps\".");
            return;
        }
        egui::ScrollArea::vertical().auto_shrink([false; 2]).show_rows(ui, ROW_H, rows.len(), |ui, range| {
            ui.spacing_mut().item_spacing.y = 0.0;
            for r in &rows[range] {
                let bg = ui.painter().add(egui::Shape::Noop);
                let inner = ui.horizontal(|ui| {
                    ui.set_height(ROW_H);
                    ui.add_space(8.0);
                    app_avatar(ui, &r.name, 26.0);
                    ui.add_space(4.0);
                    cell(ui, name_w, ROW_H, t(&r.name));
                    if show_pid {
                        cell_right(ui, W_PID, ROW_H, time_t(r.pid.to_string()));
                    }
                    cell_right(ui, W_CPU, ROW_H, tw(format!("{:.1}%", r.cpu), 13.5, W::Medium).color(if r.cpu > 15.0 { Tone::Warn.fg() } else { text() }));
                    if show_gpu {
                        let g = if r.gpu > 0.05 { format!("{:.0}%", r.gpu) } else { "-".into() };
                        cell_right(ui, W_GPU, ROW_H, t(g).color(if r.gpu > 0.05 { text() } else { muted() }));
                    }
                    cell_right(ui, W_MEM, ROW_H, muted_t(fmt_bytes(r.mem)));
                    if show_prio {
                        // only raised priorities get a chip; everything else stays quiet text
                        let tone = match r.priority {
                            "High" | "Realtime" => Some(Tone::Accent),
                            "Above normal" => Some(Tone::Neutral),
                            _ => None,
                        };
                        ui.add_space(10.0);
                        ui.allocate_ui_with_layout(vec2(W_PRIO - 10.0, ROW_H), Layout::left_to_right(Align::Center), |ui| {
                            ui.set_min_width(W_PRIO - 10.0);
                            match tone {
                                Some(t) => {
                                    chip(ui, r.priority, t);
                                }
                                None => {
                                    ui.label(small(r.priority));
                                }
                            }
                        });
                    }
                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        let w = &mut ui.visuals_mut().widgets;
                        w.inactive.weak_bg_fill = egui::Color32::TRANSPARENT;
                        w.inactive.bg_fill = egui::Color32::TRANSPARENT;
                        ui.menu_button(icon_t(icon::ELLIPSIS, 16.0).color(muted()), |ui| row_menu(ui, r, &profile_names, &mut act));
                    });
                });
                let resp = inner.response.interact(Sense::click());
                resp.context_menu(|ui| row_menu(ui, r, &profile_names, &mut act));
                let fill = if resp.hovered() { sunken() } else { egui::Color32::TRANSPARENT };
                ui.painter().set(bg, egui::Shape::rect_filled(inner.response.rect, CornerRadius::same(10), fill));
            }
        });
    });

    match act {
        Some(Act::Priority(pid, class, name)) => {
            if process::set_priority(pid, class) {
                app.toast(ToastKind::Ok, format!("{name}: priority changed"));
            } else {
                app.toast(ToastKind::Err, format!("{name}: Windows blocked the change (it's protected, or Avix needs admin rights)"));
            }
        }
        Some(Act::Efficiency(pid, on, name)) => {
            if process::set_efficiency(pid, on) {
                app.toast(ToastKind::Ok, format!("{name}: efficiency mode {}", if on { "on" } else { "off" }));
            } else {
                app.toast(ToastKind::Err, format!("{name}: Windows blocked the change (it's protected, or Avix needs admin rights)"));
            }
        }
        Some(Act::AddTrigger(pi, name)) => {
            if let Some(p) = app.cfg.profiles.get_mut(pi) {
                if !p.triggers.contains(&name) {
                    p.triggers.push(name.clone());
                }
                let pn = p.name.clone();
                app.save_config();
                app.toast(ToastKind::Ok, format!("{name} now boosts with '{pn}'"));
            }
        }
        Some(Act::NeverTouch(name)) => {
            if !app.cfg.never_touch.contains(&name) {
                app.cfg.never_touch.push(name.clone());
            }
            app.save_config();
            app.toast(ToastKind::Ok, format!("Avix will never touch {name}"));
        }
        Some(Act::OpenLocation(pid)) => match process::image_path(pid) {
            Some(path) => {
                let _ = crate::sys::hidden_cmd("explorer.exe").arg(format!("/select,{path}")).spawn();
            }
            None => app.toast(ToastKind::Err, "Couldn't read that process's path"),
        },
        Some(Act::Kill(pid, name)) => {
            app.ask(
                "End process?",
                &format!("End {name} (PID {pid})? Unsaved work in it will be lost."),
                "End process",
                true,
                move |app| {
                    if process::kill(pid) {
                        app.toast(ToastKind::Ok, format!("Ended {name}"));
                    } else {
                        app.toast(ToastKind::Err, format!("Couldn't end {name}: Windows protects it, or Avix needs admin rights"));
                    }
                },
            );
        }
        None => {}
    }
}
