use super::data::{take, Job};
use super::profiles::list_editor;
use super::theme::*;
use super::{App, ToastKind};
use crate::config::Config;
use crate::ipc::Request;
use crate::sys::{self, power};
use eframe::egui::{self, RichText, Ui};

#[derive(Default)]
pub struct State {
    autostart: Option<bool>,
    autostart_job: Option<Job<Result<bool, String>>>,
    new_never: String,
    new_ac: String,
    new_ignore: String,
    new_hog: String,
}

pub fn show(app: &mut App, ui: &mut Ui) {
    let ctx = ui.ctx().clone();
    page_header(ui, "Settings", "How the engine behaves, plus safety lists and maintenance.");

    if app.settings.autostart.is_none() && app.settings.autostart_job.is_none() {
        app.settings.autostart_job = Some(Job::spawn(&ctx, || Ok(crate::autostart::is_enabled())));
    }
    if let Some(r) = take(&mut app.settings.autostart_job) {
        match r {
            Ok(v) => app.settings.autostart = Some(v),
            Err(e) => {
                app.toast(ToastKind::Err, e);
                app.settings.autostart = Some(crate::autostart::is_enabled());
            }
        }
    }

    let profile_names: Vec<String> = app.cfg.profiles.iter().map(|p| p.name.clone()).collect();
    let elevated = app.elevated;

    card(ui, |ui| {
        section(ui, "General");
        ui.horizontal(|ui| {
            let mut on = app.settings.autostart.unwrap_or(false);
            let busy = app.settings.autostart_job.is_some() || app.settings.autostart.is_none();
            let r = ui.add_enabled_ui(elevated && !busy, |ui| toggle(ui, &mut on)).inner;
            if r.changed() {
                app.settings.autostart_job = Some(Job::spawn(&ctx, move || {
                    if on {
                        crate::autostart::enable().map(|_| true)
                    } else {
                        crate::autostart::disable().map(|_| false)
                    }
                }));
            }
            ui.vertical(|ui| {
                ui.spacing_mut().item_spacing.y = 1.0;
                ui.label("Start Avix with Windows");
                ui.label(small("Starts the tray engine at sign-in with admin rights (via Task Scheduler - no UAC prompt each boot)."));
            });
        });
        toggle_row(ui, &mut app.cfg.hotkey, "Ctrl+Alt+B toggles 'Boost now'", "Instantly boost whatever you're doing, press again to go back to auto.");
        toggle_row(ui, &mut app.cfg.notifications, "Notifications when a boost starts / ends", "Off by default - Windows hides them during fullscreen games anyway.");
        ui.horizontal(|ui| {
            ui.label("Engine check interval");
            let mut secs = app.cfg.poll_interval_ms as f32 / 1000.0;
            if ui.add(egui::Slider::new(&mut secs, 1.0..=5.0).step_by(0.5).suffix(" s")).changed() {
                app.cfg.poll_interval_ms = (secs * 1000.0) as u64;
            }
            ui.label(small("Foreground changes are instant regardless; this is for newly started apps."));
        });
    });
    ui.add_space(8.0);

    card(ui, |ui| {
        section(ui, "Auto-detect games");
        ui.horizontal(|ui| {
            toggle(ui, &mut app.cfg.auto_fullscreen);
            ui.label("Boost any app that goes fullscreen, using profile");
            egui::ComboBox::from_id_salt("fs-profile").selected_text(&app.cfg.fullscreen_profile).show_ui(ui, |ui| {
                for n in &profile_names {
                    ui.selectable_value(&mut app.cfg.fullscreen_profile, n.clone(), n);
                }
            });
        });
        ui.label(small("Catches games that aren't in a profile yet. Browsers and video players below are ignored."));
        list_editor(ui, "fs-ignore", &mut app.cfg.fullscreen_ignore, &mut app.settings.new_ignore, "app.exe", &[], true);
    });
    ui.add_space(8.0);

    card(ui, |ui| {
        section(ui, "Standby memory cleaner");
        ui.label(small("When a profile has the cleaner on, Avix empties the standby list if BOTH conditions are true (at most once a minute)."));
        ui.horizontal(|ui| {
            ui.label("Standby list is at least");
            ui.add(egui::Slider::new(&mut app.cfg.standby_min_mb, 256..=8192).step_by(256.0).suffix(" MB"));
        });
        ui.horizontal(|ui| {
            ui.label("Free memory is below");
            ui.add(egui::Slider::new(&mut app.cfg.free_memory_below_mb, 256..=8192).step_by(256.0).suffix(" MB"));
        });
    });
    ui.add_space(8.0);

    card(ui, |ui| {
        section(ui, "Latency monitor");
        ui.horizontal(|ui| {
            ui.label("Ping target");
            ui.add(egui::TextEdit::singleline(&mut app.cfg.ping_host).desired_width(220.0));
            ui.label(small("Tip: use your game's server region IP for the most accurate reading."));
        });
    });
    ui.add_space(8.0);

    card(ui, |ui| {
        section(ui, "Safety lists");
        ui.label(RichText::new("Never touch").strong());
        ui.label(small("Avix never changes these processes: Windows core, audio, anti-cheat services, voice chat and peripheral software (so your mouse/headset never lag)."));
        list_editor(ui, "never", &mut app.cfg.never_touch, &mut app.settings.new_never, "app.exe", &[], true);
        ui.add_space(8.0);
        ui.label(RichText::new("Kernel anti-cheat games").strong());
        ui.label(small("Avix never opens a handle to these games - it only optimizes everything around them. Keeps you 100% clear of anti-cheat."));
        list_editor(ui, "ac", &mut app.cfg.anticheat_protected, &mut app.settings.new_ac, "game.exe", &[], true);
        ui.add_space(8.0);
        ui.label(RichText::new("Background service hogs").strong());
        ui.label(small("System-session processes that are still throttled while boosting (indexer, update workers, telemetry runner)."));
        list_editor(ui, "hogs", &mut app.cfg.background_hogs, &mut app.settings.new_hog, "app.exe", &[], true);
    });
    ui.add_space(8.0);

    card(ui, |ui| {
        section(ui, "Maintenance");
        ui.horizontal_wrapped(|ui| {
            if ui.button("Open data folder").on_hover_text("Settings, logs and undo backups").clicked() {
                sys::elevation::shell_open(&sys::data_dir().to_string_lossy());
            }
            if ui.button("Restart engine").clicked() {
                app.restart_engine();
            }
            if ui.button("Stop engine").clicked() {
                app.ask(
                    "Stop the engine?",
                    "Avix will restore anything it changed and stop boosting until you start it again (open Avix or reboot with autostart on).",
                    "Stop",
                    true,
                    |app| {
                        app.monitor.send(Request::Shutdown);
                        app.toast(ToastKind::Info, "Engine stopped");
                    },
                );
            }
            if ui.add_enabled(elevated, egui::Button::new("Remove Avix power plan")).clicked() {
                match power::remove_avix_plan() {
                    Ok(true) => app.toast(ToastKind::Ok, "Avix power plan removed (it's recreated next boost)"),
                    Ok(false) => app.toast(ToastKind::Info, "No Avix power plan to remove"),
                    Err(e) => app.toast(ToastKind::Err, e),
                }
            }
            if ui.button(RichText::new("Reset settings to defaults").color(BAD)).clicked() {
                app.ask(
                    "Reset all settings?",
                    "Profiles and lists go back to Avix defaults. Tweaks you applied are not affected.",
                    "Reset",
                    true,
                    |app| {
                        app.cfg = Config::default();
                        app.save_config();
                    },
                );
            }
        });
    });
    ui.add_space(8.0);

    card(ui, |ui| {
        section(ui, "About");
        ui.label(RichText::new(format!("Avix {}", crate::VERSION)).strong());
        ui.label(small("Built in Rust. The engine is a tiny tray process (a few MB of RAM); this dashboard only runs while it's open."));
        ui.label(small("Avix only uses documented Windows mechanisms (priority classes, EcoQoS, power plans, services, registry policies). It never injects into games, reads or writes game memory, or installs drivers."));
    });
}
