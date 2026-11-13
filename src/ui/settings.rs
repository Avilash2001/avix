use super::data::{take, Job};
use super::profiles::list_editor;
use super::theme::*;
use super::{App, ToastKind};
use crate::config::{Config, UiTheme};
use crate::ipc::Request;
use crate::sys::{self, power};
use eframe::egui::{self, Ui};

#[derive(Default)]
pub struct State {
    /// A section search jumped to: scrolled to and outlined briefly.
    pub focus: Option<(&'static str, f64)>,
    autostart: Option<bool>,
    autostart_job: Option<Job<Result<bool, String>>>,
    new_never: String,
    new_ac: String,
    new_ignore: String,
    new_hog: String,
    /// Exe names from Windows' game list (read once per dashboard session).
    known_games: Option<Vec<String>>,
    gpus: Option<Vec<sys::hardware::Gpu>>,
}

/// A settings card that search can jump to.
fn section_card(app: &mut App, ui: &mut Ui, key: &'static str, title: &str, add: impl FnOnce(&mut App, &mut Ui)) {
    let r = ui
        .scope(|ui| {
            card(ui, |ui| {
                section(ui, title);
                ui.add_space(10.0);
                add(app, ui);
            })
        })
        .response
        .rect;
    super::search::flash(ui, r, &mut app.settings.focus, key);
    ui.add_space(14.0);
}

pub fn show(app: &mut App, ui: &mut Ui) {
    let ctx = ui.ctx().clone();
    page_header(ui, "Settings", "How the engine behaves, how the dashboard looks, plus safety lists and maintenance.");
    ui.set_max_width(ui.available_width().min(880.0));

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

    section_card(app, ui, "appearance", "Appearance", |app, ui| {
        split_row(
            ui,
            "theme",
            280.0,
            |ui| row_label(ui, "Theme", "System follows Windows' app mode. Saved right away."),
            |ui| {
                let items = [(Some(icon::SUN_MOON), "System"), (Some(icon::SUN), "Light"), (Some(icon::MOON), "Dark")];
                let sel = UiTheme::ALL.iter().position(|t| *t == app.cfg.theme).unwrap_or(0);
                if let (Some(i), _) = segmented(ui, "theme-seg", &items, sel, true) {
                    app.set_ui_prefs(Some(UiTheme::ALL[i]), None);
                }
            },
        );
        ui.add_space(10.0);
        let mut anim = app.cfg.animations;
        if toggle_row(ui, &mut anim, "Animations", "Short transitions when you switch pages, hover, or a value changes. Off makes everything instant.") {
            app.set_ui_prefs(None, Some(anim));
        }
    });

    section_card(app, ui, "general", "General", |app, ui| {
        let mut on = app.settings.autostart.unwrap_or(false);
        let busy = app.settings.autostart_job.is_some() || app.settings.autostart.is_none();
        let changed = split_row(
            ui,
            "autostart",
            44.0,
            |ui| row_label(ui, "Start Avix with Windows", "Starts the tray engine at sign-in with admin rights (via Task Scheduler — no UAC prompt each boot)."),
            |ui| ui.add_enabled_ui(elevated && !busy, |ui| toggle(ui, &mut on)).inner.changed(),
        );
        if changed {
            app.settings.autostart_job = Some(Job::spawn(&ctx, move || {
                if on {
                    crate::autostart::enable().map(|_| true)
                } else {
                    crate::autostart::disable().map(|_| false)
                }
            }));
        }
        ui.add_space(6.0);
        toggle_row(ui, &mut app.cfg.hotkey, "Ctrl+Alt+B toggles 'Boost now'", "Instantly boost whatever you're doing, press again to go back to auto.");
        toggle_row(ui, &mut app.cfg.notifications, "Notifications when a boost starts / ends", "Off by default — Windows hides them during fullscreen games anyway.");
        split_row(
            ui,
            "interval",
            260.0,
            |ui| row_label(ui, "Engine check interval", "Foreground changes are instant regardless; this is for newly started apps."),
            |ui| {
                let mut secs = app.cfg.poll_interval_ms as f64 / 1000.0;
                if slider(ui, "interval", &mut secs, 1.0..=5.0, 0.5, &|v| format!("{v:.1} s")) {
                    app.cfg.poll_interval_ms = (secs * 1000.0) as u64;
                }
            },
        );
    });

    let update = super::data::lock(&app.monitor.metrics).engine.as_ref().map(|s| s.update.clone());
    section_card(app, ui, "updates", "Updates", |app, ui| {
        toggle_row(ui, &mut app.cfg.check_updates, "Check for new versions once a day", "Asks GitHub for the latest Avix release. Nothing about your PC is sent.");
        ui.horizontal_wrapped(|ui| match &update {
            None => {
                ui.label(muted_t("Start the engine to check for updates."));
            }
            Some(u) => {
                let ago = |s: u64| match s {
                    0..=89 => "just now".to_string(),
                    90..=5399 => format!("{} min ago", (s + 30) / 60),
                    _ => format!("{} h ago", (s + 1800) / 3600),
                };
                match (&u.available, u.checking, &u.error) {
                    (_, true, _) => busy(ui, "Checking…"),
                    (Some(a), _, _) => {
                        chip_icon(ui, Some(icon::DOWNLOAD), format!("Avix {} is available (you have {})", a.version, u.current), Tone::Accent);
                        if Btn::new(Kind::Primary, "Download").small().show(ui).clicked() {
                            sys::elevation::open_url(&a.url);
                        }
                    }
                    (None, _, Some(e)) => {
                        ui.label(t(format!("Couldn't check: {e}")).color(Tone::Warn.fg()));
                    }
                    (None, _, None) => match u.checked_secs_ago {
                        Some(s) => {
                            icon_label(ui, icon::CIRCLE_CHECK, 15.0, muted());
                            ui.label(muted_t(format!("You have Avix {}, the latest (checked {})", u.current, ago(s))));
                        }
                        None => {
                            ui.label(muted_t(format!("You have Avix {}. Not checked yet.", u.current)));
                        }
                    },
                }
                if !u.checking && Btn::new(Kind::Outline, "Check now").icon(icon::REFRESH).small().show(ui).clicked() {
                    app.monitor.send(Request::CheckUpdate);
                }
            }
        });
    });

    section_card(app, ui, "detect", "Auto-detect games", |app, ui| {
        ui.add(egui::Label::new(small("For games that aren't in a profile. Detected games get the profile chosen below, whether they started before or after Avix.")).wrap());
        ui.add_space(10.0);
        let known = app
            .settings
            .known_games
            .get_or_insert_with(|| sys::games::windows_known().into_iter().map(|g| g.name).collect())
            .clone();
        let hint = match known.len() {
            0 => "Uses Windows' own game list (the one Game Bar and Game Mode use). It's empty for now — it fills as you play.".to_string(),
            n => format!("Uses Windows' own game list (the one Game Bar and Game Mode use) — {n} game{} on this PC.", if n == 1 { "" } else { "s" }),
        };
        toggle_row(ui, &mut app.cfg.windows_games, "Boost games Windows recognises", &hint);
        if !known.is_empty() {
            egui::CollapsingHeader::new(small("Show the list")).id_salt("known-games-list").show(ui, |ui| {
                ui.add(egui::Label::new(small(known.join(",  "))).wrap());
            });
            ui.add_space(6.0);
        }
        toggle_row(
            ui,
            &mut app.cfg.library_games,
            "Boost games installed from a store",
            "Anything with a window running from a Steam, Epic, GOG, Ubisoft, EA, Xbox, itch.io or Amazon Games library, or from a folder named \"Games\". Launchers and overlays are skipped.",
        );
        toggle_row(
            ui,
            &mut app.cfg.gpu_games,
            "Boost the app you're using when it keeps the GPU busy",
            "Catches games from anywhere, even windowed: after a few seconds of heavy 3D use. Ends 2 minutes after it goes idle.",
        );
        toggle_row(
            ui,
            &mut app.cfg.auto_fullscreen,
            "Boost any app that goes fullscreen",
            "Even when it isn't the window in focus. Browsers and video players below are ignored.",
        );
        split_row(ui, "fs-profile", 210.0, |ui| row_label(ui, "Profile for detected games", ""), |ui| {
            combo("fs-profile").selected_text(&app.cfg.fullscreen_profile).width(200.0).show_ui(ui, |ui| {
                for n in &profile_names {
                    ui.selectable_value(&mut app.cfg.fullscreen_profile, n.clone(), n);
                }
            });
        });
        ui.add_space(12.0);
        ui.label(strong("Never treated as games"));
        ui.add_space(4.0);
        list_editor(ui, "fs-ignore", &mut app.cfg.fullscreen_ignore, &mut app.settings.new_ignore, "app.exe", &[], true);
    });

    section_card(app, ui, "standby", "Standby memory cleaner", |app, ui| {
        ui.add(egui::Label::new(small("When a profile has the cleaner on, Avix empties the standby list if BOTH conditions are true (at most once a minute).")).wrap());
        ui.add_space(10.0);
        split_row(ui, "standby-min", 280.0, |ui| row_label(ui, "Standby list is at least", ""), |ui| {
            let mut v = app.cfg.standby_min_mb as f64;
            if slider(ui, "standby-min", &mut v, 256.0..=8192.0, 256.0, &|v| format!("{v:.0} MB")) {
                app.cfg.standby_min_mb = v as u64;
            }
        });
        ui.add_space(8.0);
        split_row(ui, "free-below", 280.0, |ui| row_label(ui, "Free memory is below", ""), |ui| {
            let mut v = app.cfg.free_memory_below_mb as f64;
            if slider(ui, "free-below", &mut v, 256.0..=8192.0, 256.0, &|v| format!("{v:.0} MB")) {
                app.cfg.free_memory_below_mb = v as u64;
            }
        });
    });

    section_card(app, ui, "latency", "Latency monitor", |app, ui| {
        split_row(
            ui,
            "ping-host",
            250.0,
            |ui| row_label(ui, "Ping target", "Tip: use your game's server region IP for the most accurate reading. Leave empty to turn the monitor off."),
            |ui| {
                text_field(ui, "ping-host", &mut app.cfg.ping_host, "1.1.1.1", 240.0, Some(icon::GLOBE));
            },
        );
        ui.add_space(6.0);
        ui.add(egui::Label::new(small("Only while the dashboard is open, Avix pings this address once a second (a standard ICMP echo, nothing else is sent).")).wrap());
    });

    section_card(app, ui, "graphics", "Graphics", |app, ui| {
        let gpus = app.settings.gpus.get_or_insert_with(sys::hardware::gpus).clone();
        let hybrid = sys::hardware::is_hybrid(&gpus);
        toggle_row(
            ui,
            &mut app.cfg.prefer_dgpu,
            "Run games on the dedicated GPU",
            "Laptops with two GPUs: Avix sets Windows' graphics preference to the dedicated GPU for each game it boosts, from that game's next launch. Choices you made yourself are never changed; turning this off removes what Avix set.",
        );
        ui.horizontal_wrapped(|ui| {
            ui.label(small("Graphics on this PC:"));
            for g in gpus.iter().filter(|g| g.real || g.basic_driver) {
                chip_icon(ui, Some(icon::LAYERS), format!("{} ({})", g.name, if g.integrated { "integrated" } else { "dedicated" }), if g.integrated { Tone::Neutral } else { Tone::Accent });
            }
        });
        if !hybrid {
            ui.add_space(4.0);
            ui.label(small("Only one GPU here, so this setting has nothing to do on this PC."));
        } else {
            let set = crate::tweaks::gpu_pref::set_by_avix();
            if !set.is_empty() {
                let names: Vec<String> = set.iter().map(|p| p.rsplit(['\\', '/']).next().unwrap_or(p).to_string()).collect();
                ui.label(small(format!("Set by Avix for: {}", names.join(", "))));
            }
        }
    });

    section_card(app, ui, "safety", "Safety lists", |app, ui| {
        ui.horizontal(|ui| {
            icon_label(ui, icon::LOCK, 15.0, muted());
            ui.label(strong("Never touch"));
        });
        ui.add(egui::Label::new(small("Avix never changes these processes: Windows core, audio, anti-cheat services, voice chat and peripheral software (so your mouse/headset never lag).")).wrap());
        ui.add_space(6.0);
        list_editor(ui, "never", &mut app.cfg.never_touch, &mut app.settings.new_never, "app.exe", &[], true);
        ui.add_space(14.0);
        divider(ui);
        ui.add_space(14.0);
        ui.horizontal(|ui| {
            icon_label(ui, icon::SHIELD_CHECK, 15.0, muted());
            ui.label(strong("Kernel anti-cheat games"));
        });
        ui.add(egui::Label::new(small("Avix never opens a handle to these games — it only optimizes everything around them. Keeps you 100% clear of anti-cheat.")).wrap());
        ui.add_space(6.0);
        list_editor(ui, "ac", &mut app.cfg.anticheat_protected, &mut app.settings.new_ac, "game.exe", &[], true);
        ui.add_space(14.0);
        divider(ui);
        ui.add_space(14.0);
        ui.horizontal(|ui| {
            icon_label(ui, icon::ARROW_DOWN, 15.0, muted());
            ui.label(strong("Background service hogs"));
        });
        ui.add(egui::Label::new(small("System-session processes that are still throttled while boosting (indexer, update workers, telemetry runner).")).wrap());
        ui.add_space(6.0);
        list_editor(ui, "hogs", &mut app.cfg.background_hogs, &mut app.settings.new_hog, "app.exe", &[], true);
    });

    section_card(app, ui, "maintenance", "Maintenance", |app, ui| {
        ui.horizontal_wrapped(|ui| {
            ui.spacing_mut().item_spacing = egui::vec2(8.0, 8.0);
            if Btn::new(Kind::Outline, "Open data folder").icon(icon::FOLDER).show(ui).on_hover_text("Settings, logs and undo backups").clicked() {
                sys::elevation::shell_open(&sys::data_dir().to_string_lossy());
            }
            if Btn::new(Kind::Outline, "Restart engine").icon(icon::REFRESH).show(ui).clicked() {
                app.restart_engine();
            }
            if Btn::new(Kind::Outline, "Stop engine").icon(icon::POWER).show(ui).clicked() {
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
            if Btn::new(Kind::Outline, "Remove Avix power plan").icon(icon::GAUGE).enabled(elevated).show(ui).clicked() {
                match power::remove_avix_plan() {
                    Ok(true) => app.toast(ToastKind::Ok, "Avix power plan removed (it's recreated next boost)"),
                    Ok(false) => app.toast(ToastKind::Info, "No Avix power plan to remove"),
                    Err(e) => app.toast(ToastKind::Err, e),
                }
            }
            if Btn::new(Kind::Danger, "Reset settings to defaults").icon(icon::UNDO).show(ui).clicked() {
                app.ask(
                    "Reset all settings?",
                    "Profiles and lists go back to Avix defaults. Tweaks you applied are not affected.",
                    "Reset",
                    true,
                    |app| {
                        let (theme, anim) = (app.cfg.theme, app.cfg.animations);
                        app.cfg = Config { theme, animations: anim, ..Config::default() };
                        app.save_config();
                    },
                );
            }
        });
    });

    section_card(app, ui, "about", "About", |_app, ui| {
        ui.label(tw(format!("Avix {}", crate::VERSION), 14.5, W::Semi));
        ui.add(egui::Label::new(small("Built in Rust. The engine is a tiny tray process (a few MB of RAM); this dashboard only runs while it's open.")).wrap());
        ui.add(egui::Label::new(small("Avix only uses documented Windows mechanisms (priority classes, EcoQoS, power plans, services, registry policies). It never injects into games, reads or writes game memory, or installs drivers.")).wrap());
        ui.add_space(4.0);
        ui.label(time_t("Type: Geist (SIL OFL 1.1). Icons: Lucide (ISC)."));
    });
}
