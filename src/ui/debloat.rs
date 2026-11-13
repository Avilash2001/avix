use super::data::{take, Job};
use super::theme::*;
use super::{App, ToastKind};
use crate::tweaks::apps::{self, InstalledPkg, RemoveResult, BLOAT};
use crate::tweaks::startup::{self, StartupItem};
use crate::tweaks::{self, catalog, Category, Restart, Risk, TweakState};
use eframe::egui::{self, Align, Layout, RichText, Ui};
use std::collections::HashSet;

#[derive(Clone, Copy, PartialEq, Eq, Default)]
pub enum Tab {
    #[default]
    Recommended,
    Cat(Category),
    Apps,
    Startup,
}

type Results = Vec<(String, Result<(), String>)>;

#[derive(Default)]
pub struct State {
    pub tab: Tab,
    states: Option<Vec<TweakState>>,
    states_job: Option<Job<Vec<TweakState>>>,
    busy: Option<Job<Results>>,
    busy_label: String,
    apps: Option<Result<Vec<InstalledPkg>, String>>,
    apps_job: Option<Job<Result<Vec<InstalledPkg>, String>>>,
    selected: HashSet<String>,
    preselected: bool,
    show_all: bool,
    remove_job: Option<Job<Result<Vec<RemoveResult>, String>>>,
    onedrive_job: Option<Job<Result<(), String>>>,
    startup: Option<Vec<StartupItem>>,
    restore_job: Option<Job<Result<(), String>>>,
}

fn state_badge(ui: &mut Ui, s: TweakState) {
    match s {
        TweakState::Applied => chip(ui, "Applied", GOOD),
        TweakState::Partial => chip(ui, "Partly applied", WARN),
        TweakState::NotApplied => chip(ui, "Not applied", MUTED),
        TweakState::NotApplicable => chip(ui, "Not on this PC", FAINT),
    };
}

fn risk_color(r: Risk) -> egui::Color32 {
    match r {
        Risk::Low => GOOD,
        Risk::Medium => WARN,
        Risk::High => BAD,
    }
}

fn refresh_states(app: &mut App, ctx: &egui::Context) {
    app.debloat.states_job = Some(Job::spawn(ctx, || catalog::all().iter().map(tweaks::state).collect()));
}

fn run_tweaks(app: &mut App, ctx: &egui::Context, ids: Vec<(&'static str, bool)>, label: &str) {
    app.debloat.busy_label = label.to_string();
    app.debloat.busy = Some(Job::spawn(ctx, move || {
        ids.into_iter()
            .filter_map(|(id, apply)| {
                let t = catalog::find(id)?;
                let r = if apply { tweaks::apply(t) } else { tweaks::revert(t) };
                Some((t.title.to_string(), r))
            })
            .collect()
    }));
}

pub fn show(app: &mut App, ui: &mut Ui) {
    let ctx = ui.ctx().clone();
    page_header(
        ui,
        "Debloat & tweaks",
        "Everything here is reversible: Avix saves the original value before it changes anything.",
    );

    // ------------------------------------------------------------ jobs
    if app.debloat.states.is_none() && app.debloat.states_job.is_none() {
        refresh_states(app, &ctx);
    }
    if let Some(s) = take(&mut app.debloat.states_job) {
        app.debloat.states = Some(s);
    }
    if let Some(results) = take(&mut app.debloat.busy) {
        let failed: Vec<String> = results.iter().filter_map(|(t, r)| r.as_ref().err().map(|e| format!("{t}: {e}"))).collect();
        let ok = results.len() - failed.len();
        if ok > 0 {
            app.toast(ToastKind::Ok, format!("{} done ({ok} item{})", app.debloat.busy_label, if ok == 1 { "" } else { "s" }));
        }
        for f in failed.into_iter().take(3) {
            app.toast(ToastKind::Err, f);
        }
        refresh_states(app, &ctx);
    }
    if let Some(r) = take(&mut app.debloat.restore_job) {
        match r {
            Ok(()) => app.toast(ToastKind::Ok, "Restore point created"),
            Err(e) => app.toast(ToastKind::Err, format!("Restore point failed: {e}")),
        }
    }

    let elevated = app.elevated;
    let busy = app.debloat.busy.is_some();
    let states = app.debloat.states.clone();
    let all = catalog::all();

    // ------------------------------------------------------------ toolbar
    let pending_rec: Vec<&'static str> = match &states {
        Some(s) => all
            .iter()
            .zip(s)
            .filter(|(t, st)| t.recommended && matches!(st, TweakState::NotApplied | TweakState::Partial))
            .map(|(t, _)| t.id)
            .collect(),
        None => Vec::new(),
    };
    ui.horizontal(|ui| {
        let restoring = app.debloat.restore_job.is_some();
        if ui
            .add_enabled(elevated && !restoring, egui::Button::new(if restoring { "Creating restore point..." } else { "Create restore point" }))
            .on_hover_text("A Windows System Restore point - an extra safety net on top of Avix's own undo")
            .clicked()
        {
            app.debloat.restore_job = Some(Job::spawn(&ctx, tweaks::create_restore_point));
        }
        let n = pending_rec.len();
        let label = format!("Apply all recommended ({n})");
        if ui.add_enabled(elevated && !busy && n > 0, egui::Button::new(RichText::new(label).strong().color(egui::Color32::WHITE)).fill(ACCENT)).clicked() {
            let ids = pending_rec.clone();
            app.ask(
                "Apply recommended tweaks?",
                &format!(
                    "Avix will apply {n} low-risk tweaks (Game DVR off, telemetry off, Start ads off, gaming scheduler tweaks...). Each one can be reverted individually. Some need a reboot to fully kick in."
                ),
                "Apply",
                false,
                move |app| {
                    let ctx = app.monitor_ctx();
                    run_tweaks(app, &ctx, ids.into_iter().map(|id| (id, true)).collect(), "Recommended tweaks");
                },
            );
        }
        if busy {
            ui.spinner();
            ui.label(muted("Working..."));
        }
    });
    ui.add_space(10.0);

    // ------------------------------------------------------------ tabs
    ui.horizontal_wrapped(|ui| {
        let tab = &mut app.debloat.tab;
        ui.selectable_value(tab, Tab::Recommended, "Recommended");
        for c in Category::ALL {
            ui.selectable_value(tab, Tab::Cat(c), c.label());
        }
        ui.separator();
        ui.selectable_value(tab, Tab::Apps, "Preinstalled apps");
        ui.selectable_value(tab, Tab::Startup, "Startup apps");
    });
    ui.add_space(10.0);

    match app.debloat.tab {
        Tab::Apps => return apps_tab(app, ui, &ctx),
        Tab::Startup => return startup_tab(app, ui),
        _ => {}
    }

    let Some(states) = states else {
        ui.horizontal(|ui| {
            ui.spinner();
            ui.label(muted("Reading current settings..."));
        });
        return;
    };

    if let Tab::Cat(c) = app.debloat.tab {
        ui.label(muted(c.blurb()));
        ui.add_space(6.0);
    } else {
        ui.label(muted("The low-risk tweaks Avix recommends for a gaming PC. Advanced ones live in their category tabs."));
        ui.add_space(6.0);
    }

    let mut todo: Option<(&'static str, bool)> = None;
    let mut ask_high: Option<&'static str> = None;
    for (t, s) in all.iter().zip(states.iter()) {
        let show = match app.debloat.tab {
            Tab::Recommended => t.recommended,
            Tab::Cat(c) => t.category == c,
            _ => false,
        };
        if !show {
            continue;
        }
        card(ui, |ui| {
            ui.horizontal(|ui| {
                ui.vertical(|ui| {
                    ui.set_max_width(ui.available_width() - 190.0);
                    ui.horizontal_wrapped(|ui| {
                        ui.label(RichText::new(t.title).strong().size(15.0));
                        state_badge(ui, *s);
                        chip(ui, t.risk.label(), risk_color(t.risk));
                        if t.recommended && app.debloat.tab != Tab::Recommended {
                            chip(ui, "Recommended", ACCENT);
                        }
                        if let Some(r) = t.restart.label() {
                            if t.restart != Restart::None {
                                chip(ui, format!("Needs {r}"), INFO);
                            }
                        }
                    });
                    ui.label(small(t.description));
                });
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    let enabled = elevated && !busy;
                    match s {
                        TweakState::Applied => {
                            if ui.add_enabled(enabled, egui::Button::new("Revert")).clicked() {
                                todo = Some((t.id, false));
                            }
                        }
                        TweakState::NotApplied | TweakState::Partial => {
                            if *s == TweakState::Partial && ui.add_enabled(enabled, egui::Button::new("Revert")).clicked() {
                                todo = Some((t.id, false));
                            }
                            let r = ui.add_enabled(enabled, egui::Button::new(RichText::new("Apply").strong().color(egui::Color32::WHITE)).fill(ACCENT));
                            if r.clicked() {
                                if t.risk == Risk::High {
                                    ask_high = Some(t.id);
                                } else {
                                    todo = Some((t.id, true));
                                }
                            }
                        }
                        TweakState::NotApplicable => {
                            ui.label(small("n/a"));
                        }
                    }
                });
            });
        });
        ui.add_space(6.0);
    }

    if let Some((id, apply)) = todo {
        run_tweaks(app, &ctx, vec![(id, apply)], if apply { "Applied" } else { "Reverted" });
    }
    if let Some(id) = ask_high {
        let t = catalog::find(id).expect("tweak");
        app.ask(
            "This is an advanced tweak",
            &format!("{}\n\n{}\n\nApply it anyway?", t.title, t.description),
            "Apply anyway",
            true,
            move |app| {
                let ctx = app.monitor_ctx();
                run_tweaks(app, &ctx, vec![(id, true)], "Applied");
            },
        );
    }
}

fn apps_tab(app: &mut App, ui: &mut Ui, ctx: &egui::Context) {
    let elevated = app.elevated;
    let st = &mut app.debloat;
    if st.apps.is_none() && st.apps_job.is_none() {
        st.apps_job = Some(Job::spawn(ctx, apps::list_installed));
    }
    if let Some(r) = take(&mut st.apps_job) {
        st.apps = Some(r);
    }
    let mut toasts: Vec<(ToastKind, String)> = Vec::new();
    if let Some(r) = take(&mut st.remove_job) {
        match r {
            Ok(results) => {
                let ok = results.iter().filter(|r| r.ok).count();
                toasts.push((ToastKind::Ok, format!("Removed {ok} of {} app(s)", results.len())));
                for f in results.iter().filter(|r| !r.ok).take(3) {
                    toasts.push((ToastKind::Err, format!("{}: {}", f.name, f.error.lines().next().unwrap_or(""))));
                }
            }
            Err(e) => toasts.push((ToastKind::Err, e)),
        }
        st.selected.clear();
        st.apps = None;
        st.apps_job = Some(Job::spawn(ctx, apps::list_installed));
    }
    if let Some(r) = take(&mut st.onedrive_job) {
        match r {
            Ok(()) => toasts.push((ToastKind::Ok, "OneDrive uninstalled".into())),
            Err(e) => toasts.push((ToastKind::Err, e)),
        }
    }

    ui.label(muted("Preinstalled Store apps you probably never asked for. Removal is for all users and stops Windows re-installing them for new accounts. You can always reinstall from the Microsoft Store."));
    ui.add_space(8.0);

    let installed = match &st.apps {
        None => {
            ui.horizontal(|ui| {
                ui.spinner();
                ui.label(muted("Scanning installed apps (takes a few seconds)..."));
            });
            for (k, t) in toasts {
                app.toast(k, t);
            }
            return;
        }
        Some(Err(e)) => {
            ui.label(RichText::new(format!("Couldn't list apps: {e}")).color(BAD));
            if ui.button("Retry").clicked() {
                st.apps = None;
            }
            for (k, t) in toasts {
                app.toast(k, t);
            }
            return;
        }
        Some(Ok(v)) => v.clone(),
    };

    let found: Vec<&apps::BloatApp> =
        BLOAT.iter().filter(|b| installed.iter().any(|p| apps::glob_match(b.pattern, &p.name))).collect();
    if !st.preselected {
        st.preselected = true;
        for b in found.iter().filter(|b| b.recommended) {
            st.selected.insert(b.pattern.to_string());
        }
    }
    let removing = st.remove_job.is_some();
    let mut want_remove: Option<Vec<String>> = None;
    let mut want_onedrive = false;

    card(ui, |ui| {
        ui.horizontal(|ui| {
            section(ui, &format!("Found {} removable apps", found.len()));
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                let n = st.selected.len();
                if removing {
                    ui.spinner();
                    ui.label(muted("Removing..."));
                } else if ui.add_enabled(elevated && n > 0, egui::Button::new(RichText::new(format!("Remove selected ({n})")).color(BAD))).clicked() {
                    want_remove = Some(st.selected.iter().cloned().collect());
                }
                if ui.small_button("Select none").clicked() {
                    st.selected.clear();
                }
                if ui.small_button("Select recommended").clicked() {
                    for b in found.iter().filter(|b| b.recommended) {
                        st.selected.insert(b.pattern.to_string());
                    }
                }
            });
        });
        if found.is_empty() {
            ui.label(muted("Nothing to remove - your install is already clean."));
        }
        egui::Grid::new("bloat").num_columns(3).spacing([16.0, 6.0]).show(ui, |ui| {
            for b in &found {
                let mut on = st.selected.contains(b.pattern);
                if ui.checkbox(&mut on, RichText::new(b.label).strong()).changed() {
                    if on {
                        st.selected.insert(b.pattern.to_string());
                    } else {
                        st.selected.remove(b.pattern);
                    }
                }
                ui.label(small(b.pattern));
                ui.horizontal(|ui| {
                    if b.recommended {
                        chip(ui, "Safe to remove", GOOD);
                    }
                    if !b.note.is_empty() {
                        ui.label(small(b.note));
                    }
                });
                ui.end_row();
            }
        });
    });
    ui.add_space(8.0);

    // OneDrive
    if apps::onedrive_uninstall_string().is_some() {
        card(ui, |ui| {
            ui.horizontal(|ui| {
                ui.vertical(|ui| {
                    ui.label(RichText::new("Microsoft OneDrive (desktop app)").strong());
                    ui.label(small("Syncs in the background and nags constantly. Files already in your OneDrive folder stay on your PC."));
                });
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    if st.onedrive_job.is_some() {
                        ui.spinner();
                    } else if ui.add_enabled(elevated, egui::Button::new(RichText::new("Uninstall OneDrive").color(BAD))).clicked() {
                        want_onedrive = true;
                    }
                });
            });
        });
        ui.add_space(8.0);
    }

    // Everything else (advanced)
    let st = &mut app.debloat;
    card(ui, |ui| {
        ui.horizontal(|ui| {
            toggle(ui, &mut st.show_all);
            ui.label("Show every other installed app (advanced)");
        });
        if st.show_all {
            ui.label(RichText::new("Only remove things you recognise. System components are hidden or marked non-removable.").color(WARN).size(12.5));
            let others: Vec<&InstalledPkg> = installed
                .iter()
                .filter(|p| !p.non_removable.unwrap_or(false) && !BLOAT.iter().any(|b| apps::glob_match(b.pattern, &p.name)))
                .collect();
            egui::Grid::new("others").num_columns(2).spacing([16.0, 4.0]).show(ui, |ui| {
                for p in others {
                    let mut on = st.selected.contains(&p.name);
                    if ui.checkbox(&mut on, &p.name).changed() {
                        if on {
                            st.selected.insert(p.name.clone());
                        } else {
                            st.selected.remove(&p.name);
                        }
                    }
                    ui.label("");
                    ui.end_row();
                }
            });
        }
    });

    for (k, t) in toasts {
        app.toast(k, t);
    }
    if let Some(names) = want_remove {
        let list = names.join(", ");
        let n = names.len();
        let ctx2 = ctx.clone();
        app.ask(
            "Remove these apps?",
            &format!("This removes {n} app(s) for all users:\n{list}\n\nYou can reinstall any of them from the Microsoft Store."),
            "Remove",
            true,
            move |app| {
                app.debloat.remove_job = Some(Job::spawn(&ctx2, move || apps::remove(&names)));
            },
        );
    }
    if want_onedrive {
        let ctx2 = ctx.clone();
        app.ask(
            "Uninstall OneDrive?",
            "OneDrive will stop syncing and be uninstalled. Your local files stay where they are. You can reinstall it from microsoft.com any time.",
            "Uninstall",
            true,
            move |app| {
                app.debloat.onedrive_job = Some(Job::spawn(&ctx2, apps::uninstall_onedrive));
            },
        );
    }
}

fn startup_tab(app: &mut App, ui: &mut Ui) {
    if app.debloat.startup.is_none() {
        app.debloat.startup = Some(startup::list());
    }
    ui.horizontal(|ui| {
        ui.label(muted("Apps that launch when you sign in. Disabling uses the same switch as Task Manager, so it's fully reversible."));
        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
            if ui.button("Refresh").clicked() {
                app.debloat.startup = Some(startup::list());
            }
        });
    });
    ui.add_space(8.0);
    let items = app.debloat.startup.clone().unwrap_or_default();
    let enabled_n = items.iter().filter(|i| i.enabled).count();
    let mut result: Option<Result<String, String>> = None;
    card(ui, |ui| {
        section(ui, &format!("{enabled_n} of {} enabled", items.len()));
        egui::Grid::new("startup").num_columns(3).spacing([14.0, 8.0]).show(ui, |ui| {
            for it in &items {
                let mut on = it.enabled;
                let machine = !matches!(it.source, startup::Source::RunUser | startup::Source::FolderUser);
                let r = ui.add_enabled_ui(!machine || app.elevated, |ui| toggle(ui, &mut on)).inner;
                if r.changed() {
                    result = Some(
                        startup::set_enabled(it, on)
                            .map(|_| format!("{} {}", it.name, if on { "enabled" } else { "disabled" }))
                            .map_err(|e| format!("{}: {e}", it.name)),
                    );
                }
                ui.vertical(|ui| {
                    ui.spacing_mut().item_spacing.y = 1.0;
                    ui.label(RichText::new(&it.name).strong().color(if it.enabled { TEXT } else { MUTED }));
                    ui.add(egui::Label::new(small(&it.command)).truncate());
                });
                chip(ui, it.source.label(), if machine { INFO } else { MUTED });
                ui.end_row();
            }
        });
    });
    if let Some(r) = result {
        match r {
            Ok(m) => app.toast(ToastKind::Ok, m),
            Err(e) => app.toast(ToastKind::Err, e),
        }
        app.debloat.startup = Some(startup::list());
    }
}
