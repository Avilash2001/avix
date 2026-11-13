use super::data::{lock, ProcRow};
use super::theme::*;
use super::{App, ToastKind};
use crate::sys::{fmt_bytes, process};
use eframe::egui::{self, vec2, Align, CornerRadius, Layout, RichText, Sense, Ui};

#[derive(Clone, Copy, PartialEq, Eq, Default)]
pub enum Sort {
    #[default]
    Cpu,
    Gpu,
    Memory,
    Name,
}

pub struct State {
    filter: String,
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

const W_PID: f32 = 70.0;
const W_CPU: f32 = 70.0;
const W_GPU: f32 = 70.0;
const W_MEM: f32 = 90.0;
const W_PRIO: f32 = 110.0;
const W_ACT: f32 = 44.0;

fn header(ui: &mut Ui, label: &str, sort: Sort, cur: &mut Sort, w: f32) {
    let text = if *cur == sort { RichText::new(format!("{label} v")).strong().color(ACCENT) } else { RichText::new(label).color(MUTED) };
    let r = ui
        .allocate_ui_with_layout(vec2(w, 22.0), Layout::left_to_right(Align::Center), |ui| {
            ui.set_min_size(vec2(w, 22.0));
            ui.add(egui::Label::new(text.size(12.5)).sense(Sense::click()))
        })
        .inner;
    if r.on_hover_cursor(egui::CursorIcon::PointingHand).clicked() {
        *cur = sort;
    }
}

pub fn show(app: &mut App, ui: &mut Ui) {
    page_header(ui, "Processes", "Live view of everything running. Use the ... menu on a row for actions.");

    let st = &mut app.processes;
    ui.horizontal(|ui| {
        ui.add(egui::TextEdit::singleline(&mut st.filter).hint_text("Search processes...").desired_width(280.0));
        ui.add_space(8.0);
        toggle(ui, &mut st.only_mine);
        ui.label("Only my apps");
    });
    ui.add_space(8.0);

    let mut rows: Vec<ProcRow> = {
        let m = lock(&app.monitor.metrics);
        let f = st.filter.to_lowercase();
        m.procs
            .iter()
            .filter(|p| (!st.only_mine || p.user_session) && (f.is_empty() || p.name_lc.contains(&f) || p.pid.to_string() == f))
            .cloned()
            .collect()
    };
    match st.sort {
        Sort::Cpu => rows.sort_by(|a, b| b.cpu.total_cmp(&a.cpu).then(b.mem.cmp(&a.mem))),
        Sort::Gpu => rows.sort_by(|a, b| b.gpu.total_cmp(&a.gpu).then(b.cpu.total_cmp(&a.cpu))),
        Sort::Memory => rows.sort_by(|a, b| b.mem.cmp(&a.mem)),
        Sort::Name => rows.sort_by(|a, b| a.name_lc.cmp(&b.name_lc)),
    }

    let profile_names: Vec<String> = app.cfg.profiles.iter().map(|p| p.name.clone()).collect();
    let mut act: Option<Act> = None;

    egui::Frame::new()
        .fill(CARD)
        .stroke(egui::Stroke::new(1.0_f32, BORDER))
        .corner_radius(CornerRadius::same(12))
        .inner_margin(egui::Margin::symmetric(14, 10))
        .show(ui, |ui| {
            ui.set_width(ui.available_width());
            let name_w = (ui.available_width() - W_PID - W_CPU - W_GPU - W_MEM - W_PRIO - W_ACT - 6.0 * 8.0).max(160.0);
            ui.horizontal(|ui| {
                header(ui, "Name", Sort::Name, &mut st.sort, name_w);
                ui.add_sized([W_PID, 22.0], egui::Label::new(RichText::new("PID").size(12.5).color(MUTED)));
                header(ui, "CPU", Sort::Cpu, &mut st.sort, W_CPU);
                header(ui, "GPU", Sort::Gpu, &mut st.sort, W_GPU);
                header(ui, "Memory", Sort::Memory, &mut st.sort, W_MEM);
                ui.add_sized([W_PRIO, 22.0], egui::Label::new(RichText::new("Priority").size(12.5).color(MUTED)));
            });
            ui.separator();
            let row_h = 28.0;
            egui::ScrollArea::vertical().auto_shrink([false; 2]).show_rows(ui, row_h, rows.len(), |ui, range| {
                for (i, r) in rows[range.clone()].iter().enumerate() {
                    let bg = ui.painter().add(egui::Shape::Noop);
                    let resp = ui.horizontal(|ui| {
                        ui.set_height(row_h - 4.0);
                        cell(ui, name_w, row_h - 4.0, &r.name);
                        ui.add_sized([W_PID, row_h - 4.0], egui::Label::new(small(r.pid.to_string())));
                        ui.add_sized(
                            [W_CPU, row_h - 4.0],
                            egui::Label::new(RichText::new(format!("{:.1}%", r.cpu)).color(if r.cpu > 15.0 { WARN } else { TEXT })),
                        );
                        ui.add_sized(
                            [W_GPU, row_h - 4.0],
                            egui::Label::new(RichText::new(if r.gpu > 0.05 { format!("{:.0}%", r.gpu) } else { "-".into() }).color(if r.gpu > 30.0 { GOOD } else { MUTED })),
                        );
                        ui.add_sized([W_MEM, row_h - 4.0], egui::Label::new(fmt_bytes(r.mem)));
                        let pc = match r.priority {
                            "High" | "Realtime" => GOOD,
                            "Above normal" => INFO,
                            "Below normal" | "Low" => FAINT,
                            _ => MUTED,
                        };
                        ui.add_sized([W_PRIO, row_h - 4.0], egui::Label::new(RichText::new(r.priority).size(12.5).color(pc)));
                        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                            ui.menu_button("...", |ui| {
                                ui.label(RichText::new(&r.name).strong());
                                ui.separator();
                                ui.menu_button("Set priority", |ui| {
                                    for (class, label) in process::USER_PRIORITIES {
                                        if ui.button(label).clicked() {
                                            act = Some(Act::Priority(r.pid, class, r.name.clone()));
                                            ui.close_menu();
                                        }
                                    }
                                });
                                if ui.button("Efficiency mode on").clicked() {
                                    act = Some(Act::Efficiency(r.pid, true, r.name.clone()));
                                    ui.close_menu();
                                }
                                if ui.button("Efficiency mode off").clicked() {
                                    act = Some(Act::Efficiency(r.pid, false, r.name.clone()));
                                    ui.close_menu();
                                }
                                ui.separator();
                                ui.menu_button("Boost with profile", |ui| {
                                    for (pi, n) in profile_names.iter().enumerate() {
                                        if ui.button(n).clicked() {
                                            act = Some(Act::AddTrigger(pi, r.name_lc.clone()));
                                            ui.close_menu();
                                        }
                                    }
                                });
                                if ui.button("Never touch this app").clicked() {
                                    act = Some(Act::NeverTouch(r.name_lc.clone()));
                                    ui.close_menu();
                                }
                                if ui.button("Open file location").clicked() {
                                    act = Some(Act::OpenLocation(r.pid));
                                    ui.close_menu();
                                }
                                ui.separator();
                                if ui.button(RichText::new("End process").color(BAD)).clicked() {
                                    act = Some(Act::Kill(r.pid, r.name.clone()));
                                    ui.close_menu();
                                }
                            });
                        });
                    });
                    let rect = resp.response.rect.expand2(vec2(6.0, 1.0));
                    let fill = if resp.response.hovered() {
                        CARD_HI
                    } else if (range.start + i) % 2 == 1 {
                        tint(CARD_HI, 110)
                    } else {
                        egui::Color32::TRANSPARENT
                    };
                    ui.painter().set(bg, egui::Shape::rect_filled(rect, CornerRadius::same(6), fill));
                }
            });
        });

    match act {
        Some(Act::Priority(pid, class, name)) => {
            if process::set_priority(pid, class) {
                app.toast(ToastKind::Ok, format!("{name}: priority changed"));
            } else {
                app.toast(ToastKind::Err, format!("{name}: access denied (protected or needs admin)"));
            }
        }
        Some(Act::Efficiency(pid, on, name)) => {
            if process::set_efficiency(pid, on) {
                app.toast(ToastKind::Ok, format!("{name}: efficiency mode {}", if on { "on" } else { "off" }));
            } else {
                app.toast(ToastKind::Err, format!("{name}: access denied"));
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
                        app.toast(ToastKind::Err, format!("Couldn't end {name} (access denied)"));
                    }
                },
            );
        }
        None => {}
    }
}
