use super::dashboard::app_avatar;
use super::data::{take, Job};
use super::theme::*;
use super::{App, ToastKind};
use crate::tweaks::apps::{self, InstalledPkg, RemoveResult, BLOAT};
use crate::tweaks::startup::{self, StartupItem};
use crate::tweaks::{self, catalog, Category, Restart, Risk, TweakState};
use eframe::egui::{self, Ui};
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
    /// A tweak search jumped to: scrolled to and outlined briefly.
    pub focus: Option<(&'static str, f64)>,
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

fn state_chip(ui: &mut Ui, s: TweakState) {
    match s {
        TweakState::Applied => chip_icon(ui, Some(icon::CHECK), "Applied", Tone::Accent),
        TweakState::Partial => chip(ui, "Partly applied", Tone::Warn),
        TweakState::NotApplied => chip(ui, "Not applied", Tone::Neutral),
        TweakState::NotApplicable => chip(ui, "Not on this PC", Tone::Neutral),
    };
}

fn risk_tone(r: Risk) -> Tone {
    match r {
        Risk::Low => Tone::Neutral,
        Risk::Medium => Tone::Warn,
        Risk::High => Tone::Bad,
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
    let working = app.debloat.busy.is_some();
    let states = app.debloat.states.clone();
    let all = catalog::all();

    // ------------------------------------------------------------ header + toolbar
    let pending_rec: Vec<&'static str> = match &states {
        Some(s) => all
            .iter()
            .zip(s)
            .filter(|(t, st)| t.recommended && matches!(st, TweakState::NotApplied | TweakState::Partial))
            .map(|(t, _)| t.id)
            .collect(),
        None => Vec::new(),
    };
    let header = |ui: &mut Ui| {
        ui.label(tw("Debloat & tweaks", 26.0, W::Semi).extra_letter_spacing(-0.5));
        ui.add(egui::Label::new(muted_t("Everything here is reversible: Avix saves the original value before it changes anything.")).wrap());
    };
    let actions = |app: &mut App, ui: &mut Ui| {
        let n = pending_rec.len();
        if Btn::new(Kind::Primary, &format!("Apply all recommended ({n})"))
            .icon(icon::SPARKLES)
            .enabled(elevated && !working && n > 0)
            .show(ui)
            .clicked()
        {
            let ids = pending_rec.clone();
            app.ask(
                "Apply recommended tweaks?",
                &format!(
                    "Avix will apply {n} low-risk tweaks (Game DVR off, telemetry off, Start ads off, gaming scheduler tweaks…). Each one can be reverted individually. Some need a reboot to fully kick in."
                ),
                "Apply",
                false,
                move |app| {
                    let ctx = app.monitor_ctx();
                    run_tweaks(app, &ctx, ids.into_iter().map(|id| (id, true)).collect(), "Recommended tweaks");
                },
            );
        }
        let restoring = app.debloat.restore_job.is_some();
        if Btn::new(Kind::Outline, if restoring { "Creating restore point…" } else { "Create restore point" })
            .icon(icon::SHIELD_CHECK)
            .enabled(elevated && !restoring)
            .show(ui)
            .on_hover_text("A Windows System Restore point — an extra safety net on top of Avix's own undo")
            .clicked()
        {
            app.debloat.restore_job = Some(Job::spawn(&ctx, tweaks::create_restore_point));
        }
    };
    if ui.available_width() > 900.0 {
        split_row(ui, "debloat-head", 430.0, |ui| header(ui), |ui| actions(app, ui));
    } else {
        header(ui);
        ui.add_space(10.0);
        ui.horizontal_wrapped(|ui| actions(app, ui));
    }
    ui.add_space(16.0);

    // ------------------------------------------------------------ tabs
    ui.horizontal_wrapped(|ui| {
        ui.spacing_mut().item_spacing = egui::vec2(6.0, 8.0);
        let count = |f: &dyn Fn(&tweaks::Tweak) -> bool| all.iter().filter(|t| f(t)).count();
        let tab = &mut app.debloat.tab;
        if pill_tab(ui, "Recommended", *tab == Tab::Recommended, Some(count(&|t| t.recommended))).clicked() {
            *tab = Tab::Recommended;
        }
        for c in Category::ALL {
            if pill_tab(ui, c.label(), *tab == Tab::Cat(c), Some(count(&|t| t.category == c))).clicked() {
                *tab = Tab::Cat(c);
            }
        }
        ui.add_space(8.0);
        if pill_tab(ui, "Preinstalled apps", *tab == Tab::Apps, None).clicked() {
            *tab = Tab::Apps;
        }
        if pill_tab(ui, "Startup apps", *tab == Tab::Startup, None).clicked() {
            *tab = Tab::Startup;
        }
    });
    ui.add_space(14.0);

    match app.debloat.tab {
        Tab::Apps => return apps_tab(app, ui, &ctx),
        Tab::Startup => return startup_tab(app, ui),
        _ => {}
    }

    let Some(states) = states else {
        card(ui, |ui| busy(ui, "Reading current settings…"));
        return;
    };

    let shown: Vec<(&tweaks::Tweak, TweakState)> = all
        .iter()
        .zip(states.iter())
        .filter(|(t, _)| match app.debloat.tab {
            Tab::Recommended => t.recommended,
            Tab::Cat(c) => t.category == c,
            _ => false,
        })
        .map(|(t, s)| (t, *s))
        .collect();
    let applied = shown.iter().filter(|(_, s)| *s == TweakState::Applied).count();
    let blurb = match app.debloat.tab {
        Tab::Cat(c) => c.blurb(),
        _ => "The low-risk tweaks Avix recommends for a gaming PC. Advanced ones live in their category tabs.",
    };

    let mut todo: Option<(&'static str, bool)> = None;
    let mut ask_high: Option<&'static str> = None;
    card(ui, |ui| {
        split_row(
            ui,
            "tweak-summary",
            150.0,
            |ui| {
                ui.add(egui::Label::new(muted_t(blurb)).wrap());
            },
            |ui| {
                ui.label(tw(format!("{applied} of {} applied", shown.len()), 13.5, W::Medium));
                if working {
                    ui.add(egui::Spinner::new().size(14.0).color(accent()));
                }
            },
        );
        for (t, s) in &shown {
            ui.add_space(12.0);
            divider(ui);
            ui.add_space(12.0);
            let top = ui.cursor().min;
            split_row(
                ui,
                t.id,
                190.0,
                |ui| {
                    ui.horizontal_wrapped(|ui| {
                        ui.spacing_mut().item_spacing.x = 6.0;
                        ui.label(tw(t.title, 14.5, W::Semi));
                        ui.add_space(2.0);
                        state_chip(ui, *s);
                        chip(ui, t.risk.label(), risk_tone(t.risk));
                        if t.recommended && app.debloat.tab != Tab::Recommended {
                            chip(ui, "Recommended", Tone::Accent);
                        }
                        if let Some(r) = t.restart.label() {
                            if t.restart != Restart::None {
                                chip_icon(ui, Some(icon::REFRESH), format!("Needs {r}"), Tone::Neutral);
                            }
                        }
                    });
                    ui.add_space(2.0);
                    ui.add(egui::Label::new(small(t.description)).wrap());
                },
                |ui| {
                    let enabled = elevated && !working;
                    match s {
                        TweakState::Applied => {
                            if Btn::new(Kind::Outline, "Revert").icon(icon::UNDO).enabled(enabled).show(ui).clicked() {
                                todo = Some((t.id, false));
                            }
                        }
                        TweakState::NotApplied | TweakState::Partial => {
                            if Btn::new(Kind::Secondary, "Apply").enabled(enabled).min_width(84.0).show(ui).clicked() {
                                if t.risk == Risk::High {
                                    ask_high = Some(t.id);
                                } else {
                                    todo = Some((t.id, true));
                                }
                            }
                            if *s == TweakState::Partial && Btn::new(Kind::Ghost, "Revert").enabled(enabled).show(ui).clicked() {
                                todo = Some((t.id, false));
                            }
                        }
                        TweakState::NotApplicable => {
                            ui.label(small("Nothing to do"));
                        }
                    }
                },
            );
            let rect = egui::Rect::from_min_max(top - egui::vec2(8.0, 8.0), egui::pos2(top.x + ui.available_width() + 8.0, ui.cursor().min.y + 4.0));
            super::search::flash(ui, rect, &mut app.debloat.focus, t.id);
        }
    });

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

    let installed = match &st.apps {
        None => {
            card(ui, |ui| busy(ui, "Scanning installed apps (takes a few seconds)…"));
            for (k, t) in toasts {
                app.toast(k, t);
            }
            return;
        }
        Some(Err(e)) => {
            let e = e.clone();
            card_tinted(ui, Tone::Bad, |ui| {
                ui.label(strong(format!("Couldn't list apps: {e}")));
                ui.add_space(6.0);
                if secondary(ui, "Retry").clicked() {
                    st.apps = None;
                }
            });
            for (k, t) in toasts {
                app.toast(k, t);
            }
            return;
        }
        Some(Ok(v)) => v.clone(),
    };

    let found: Vec<&apps::BloatApp> = BLOAT.iter().filter(|b| installed.iter().any(|p| apps::glob_match(b.pattern, &p.name))).collect();
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
        split_row(
            ui,
            "apps-head",
            ui.available_width().min(460.0) * 0.8,
            |ui| {
                section(ui, &format!("{} removable apps found", found.len()));
                ui.add(egui::Label::new(small("Store apps you probably never asked for. Removal is for all users and stops Windows re-installing them for new accounts; you can always reinstall from the Microsoft Store.")).wrap());
            },
            |ui| {
                let n = st.selected.len();
                if removing {
                    busy(ui, "Removing…");
                } else if Btn::new(Kind::Danger, &format!("Remove selected ({n})")).icon(icon::TRASH).enabled(elevated && n > 0).show(ui).clicked() {
                    want_remove = Some(st.selected.iter().cloned().collect());
                }
                if Btn::new(Kind::Ghost, "None").small().show(ui).clicked() {
                    st.selected.clear();
                }
                if Btn::new(Kind::Ghost, "Recommended").small().show(ui).clicked() {
                    for b in found.iter().filter(|b| b.recommended) {
                        st.selected.insert(b.pattern.to_string());
                    }
                }
            },
        );
        ui.add_space(8.0);
        if found.is_empty() {
            empty_state(ui, icon::CIRCLE_CHECK, "Nothing to remove", "Your install is already clean.");
        }
        let cols = if ui.available_width() > 820.0 { 2 } else { 1 };
        egui::Grid::new("bloat").num_columns(cols).spacing([28.0, 4.0]).min_col_width((ui.available_width() - 28.0) / cols as f32).show(ui, |ui| {
            for (i, b) in found.iter().enumerate() {
                ui.horizontal(|ui| {
                    ui.set_min_height(38.0);
                    let mut on = st.selected.contains(b.pattern);
                    if checkbox(ui, &mut on, strong(b.label)).changed() {
                        if on {
                            st.selected.insert(b.pattern.to_string());
                        } else {
                            st.selected.remove(b.pattern);
                        }
                    }
                    if b.recommended {
                        chip(ui, "Safe to remove", Tone::Neutral);
                    }
                    let tip = if b.note.is_empty() { b.pattern.to_string() } else { format!("{}\n{}", b.note, b.pattern) };
                    icon_label(ui, icon::INFO, 14.0, faint()).on_hover_text(tip);
                });
                if (i + 1) % cols == 0 {
                    ui.end_row();
                }
            }
        });
    });
    ui.add_space(14.0);

    if apps::onedrive_uninstall_string().is_some() {
        card(ui, |ui| {
            split_row(
                ui,
                "onedrive",
                190.0,
                |ui| {
                    ui.label(strong("Microsoft OneDrive (desktop app)"));
                    ui.add(egui::Label::new(small("Syncs in the background and nags constantly. Files already in your OneDrive folder stay on your PC.")).wrap());
                },
                |ui| {
                    if st.onedrive_job.is_some() {
                        ui.add(egui::Spinner::new().color(accent()));
                    } else if Btn::new(Kind::Danger, "Uninstall OneDrive").enabled(elevated).show(ui).clicked() {
                        want_onedrive = true;
                    }
                },
            );
        });
        ui.add_space(14.0);
    }

    let st = &mut app.debloat;
    card(ui, |ui| {
        toggle_row(ui, &mut st.show_all, "Show every other installed app", "Advanced. Only remove things you recognise; system components are hidden.");
        if st.show_all {
            ui.add_space(4.0);
            let others: Vec<&InstalledPkg> = installed
                .iter()
                .filter(|p| !p.non_removable.unwrap_or(false) && !BLOAT.iter().any(|b| apps::glob_match(b.pattern, &p.name)))
                .collect();
            let cols = if ui.available_width() > 820.0 { 2 } else { 1 };
            egui::Grid::new("others").num_columns(cols).spacing([28.0, 8.0]).show(ui, |ui| {
                for (i, p) in others.iter().enumerate() {
                    let mut on = st.selected.contains(&p.name);
                    if checkbox(ui, &mut on, t(&p.name)).changed() {
                        if on {
                            st.selected.insert(p.name.clone());
                        } else {
                            st.selected.remove(&p.name);
                        }
                    }
                    if (i + 1) % cols == 0 {
                        ui.end_row();
                    }
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
    let items = app.debloat.startup.clone().unwrap_or_default();
    let enabled_n = items.iter().filter(|i| i.enabled).count();
    let mut result: Option<Result<String, String>> = None;
    let mut refresh = false;
    card(ui, |ui| {
        split_row(
            ui,
            "startup-head",
            120.0,
            |ui| {
                section(ui, &format!("{enabled_n} of {} start with Windows", items.len()));
                ui.add(egui::Label::new(small("Apps that launch when you sign in. Turning one off uses the same switch as Task Manager, so it's fully reversible.")).wrap());
            },
            |ui| {
                if Btn::new(Kind::Outline, "Refresh").icon(icon::REFRESH).small().show(ui).clicked() {
                    refresh = true;
                }
            },
        );
        if items.is_empty() {
            empty_state(ui, icon::CIRCLE_CHECK, "No startup apps", "Nothing launches at sign-in.");
        }
        for it in &items {
            ui.add_space(8.0);
            divider(ui);
            ui.add_space(8.0);
            let machine = !matches!(it.source, startup::Source::RunUser | startup::Source::FolderUser);
            split_row(
                ui,
                (&it.name, &it.command),
                56.0,
                |ui| {
                    ui.horizontal(|ui| {
                        app_avatar(ui, &it.name, 28.0);
                        ui.vertical(|ui| {
                            ui.spacing_mut().item_spacing.y = 1.0;
                            ui.horizontal(|ui| {
                                ui.label(strong(&it.name).color(if it.enabled { text() } else { muted() }));
                                chip(ui, it.source.label(), if machine { Tone::Accent } else { Tone::Neutral });
                            });
                            ui.add(egui::Label::new(time_t(&it.command)).truncate());
                        });
                    });
                },
                |ui| {
                    let mut on = it.enabled;
                    let r = ui.add_enabled_ui(!machine || app.elevated, |ui| toggle(ui, &mut on)).inner;
                    if r.changed() {
                        result = Some(
                            startup::set_enabled(it, on)
                                .map(|_| format!("{} {}", it.name, if on { "enabled" } else { "disabled" }))
                                .map_err(|e| format!("{}: {e}", it.name)),
                        );
                    }
                },
            );
        }
    });
    if refresh {
        app.debloat.startup = Some(startup::list());
    }
    if let Some(r) = result {
        match r {
            Ok(m) => app.toast(ToastKind::Ok, m),
            Err(e) => app.toast(ToastKind::Err, e),
        }
        app.debloat.startup = Some(startup::list());
    }
}
