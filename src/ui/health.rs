use super::data::{lock, take, Job};
use super::theme::*;
use super::{App, Page, ToastKind};
use crate::health::{self, Finding, Fix, Severity};
use crate::sys::{display, net};
use crate::tweaks::{self, catalog};
use eframe::egui::{self, vec2, Align, Layout, Ui};

#[derive(Default)]
pub struct State {
    findings: Option<Vec<Finding>>,
    adapters: Vec<net::AdapterInfo>,
    job: Option<Job<(Vec<Finding>, Vec<net::AdapterInfo>)>>,
    fix_job: Option<Job<Result<String, String>>>,
}

fn sev(s: Severity) -> (Tone, char) {
    match s {
        Severity::Bad => (Tone::Bad, icon::CIRCLE_ALERT),
        Severity::Warn => (Tone::Warn, icon::ALERT),
        Severity::Info => (Tone::Accent, icon::INFO),
        Severity::Good => (Tone::Neutral, icon::CIRCLE_CHECK),
    }
}

fn scan(app: &mut App, ctx: &egui::Context) {
    app.health.job = Some(Job::spawn(ctx, || (health::scan(), net::active_adapters())));
}

fn stat(ui: &mut Ui, w: f32, label: &str, value: String, tone: Option<Tone>) {
    ui.allocate_ui_with_layout(vec2(w, 54.0), Layout::top_down(Align::Min), |ui| {
        ui.set_width(w);
        ui.spacing_mut().item_spacing.y = 2.0;
        ui.label(label_t(label));
        ui.label(tw(value, 22.0, W::Semi).color(tone.map(|t| t.fg()).unwrap_or(text())));
    });
}

pub fn show(app: &mut App, ui: &mut Ui) {
    let ctx = ui.ctx().clone();

    if app.health.findings.is_none() && app.health.job.is_none() {
        scan(app, &ctx);
    }
    if let Some((f, a)) = take(&mut app.health.job) {
        app.health.findings = Some(f);
        app.health.adapters = a;
    }
    if let Some(r) = take(&mut app.health.fix_job) {
        match r {
            Ok(m) => app.toast(ToastKind::Ok, m),
            Err(e) => app.toast(ToastKind::Err, e),
        }
        scan(app, &ctx);
    }

    let scanning = app.health.job.is_some();
    split_row(
        ui,
        "health-head",
        150.0,
        |ui| {
            ui.label(tw("Health check", 26.0, W::Semi).extra_letter_spacing(-0.5));
            ui.add(egui::Label::new(muted_t("Finds the usual causes of stutter, input lag and network lag on this PC.")).wrap());
        },
        |ui| {
            if Btn::new(Kind::Outline, if scanning { "Scanning…" } else { "Scan again" }).icon(icon::REFRESH).enabled(!scanning).show(ui).clicked() {
                scan(app, &ctx);
            }
        },
    );
    ui.add_space(16.0);

    // ------------------------------------------------------------ connection (live)
    let (stats, target, ping) = {
        let m = lock(&app.monitor.metrics);
        (m.ping_stats(), m.ping_target.clone(), m.ping_ms)
    };
    card(ui, |ui| {
        section_with(ui, "Connection", |ui| {
            ui.label(small(if target.is_empty() { "Latency monitor off".to_string() } else { format!("Pinging {target}") }));
        });
        ui.add_space(8.0);
        ui.horizontal_wrapped(|ui| {
            for a in &app.health.adapters {
                let slow = a.wifi || (a.speed_mbps > 0 && a.speed_mbps < 1000);
                let why = if a.wifi {
                    "Wi-Fi adds latency spikes and jitter compared to a cable."
                } else if slow {
                    "Linked at less than 1000 Mbps: usually a damaged or old cable, or a slow port."
                } else {
                    "Wired gigabit link."
                };
                let label = format!("{} · {} · {} Mbps{}", a.name, if a.wifi { "Wi-Fi" } else { "Wired" }, a.speed_mbps, if slow && !a.wifi { " · slow link" } else { "" });
                chip_icon(ui, Some(if a.wifi { icon::WIFI } else { icon::NETWORK }), label, if slow { Tone::Warn } else { Tone::Neutral }).on_hover_text(why);
            }
            if app.health.adapters.is_empty() && !scanning {
                ui.label(small("No active network adapter found."));
            }
        });
        ui.add_space(12.0);
        let w = ui.available_width();
        let cols = if w > 560.0 { 4 } else { 2 };
        let cw = (w - 12.0 * (cols as f32 - 1.0)) / cols as f32;
        let loss_tone = if stats.loss > 2.0 { Some(Tone::Bad) } else { None };
        let jitter_tone = if stats.jitter > 10.0 { Some(Tone::Warn) } else { None };
        egui::Grid::new("ping-stats").num_columns(cols).spacing([12.0, 10.0]).show(ui, |ui| {
            stat(ui, cw, "Ping now", ping.map(|p| format!("{p} ms")).unwrap_or_else(|| "-".into()), None);
            stat(ui, cw, "Average", format!("{:.0} ms", stats.avg), None);
            if cols == 2 {
                ui.end_row();
            }
            stat(ui, cw, "Jitter", format!("{:.1} ms", stats.jitter), jitter_tone);
            stat(ui, cw, "Packet loss", format!("{:.1}%", stats.loss), loss_tone);
            ui.end_row();
        });
        ui.add_space(4.0);
        let (txt, tone) = if target.is_empty() {
            ("Turn the latency monitor on in Settings to check jitter and packet loss.", Tone::Neutral)
        } else if stats.samples < 10 {
            ("Collecting samples…", Tone::Neutral)
        } else if stats.loss > 2.0 {
            ("Packet loss detected — check Wi-Fi signal / cable / router, or contact your ISP.", Tone::Bad)
        } else if stats.jitter > 10.0 {
            ("High jitter — something on your network (downloads, streaming, Wi-Fi) is competing for bandwidth.", Tone::Warn)
        } else {
            ("Stable connection.", Tone::Neutral)
        };
        ui.horizontal(|ui| {
            status_dot(ui, tone.solid(), 3.5);
            ui.add(egui::Label::new(t(txt).color(if tone == Tone::Neutral { muted() } else { tone.fg() })).wrap());
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| ui.label(time_t(format!("{} samples", stats.samples))));
        });
    });
    ui.add_space(14.0);

    let Some(findings) = app.health.findings.clone() else {
        card(ui, |ui| busy(ui, "Scanning this PC…"));
        return;
    };
    let bad = findings.iter().filter(|f| f.severity == Severity::Bad).count();
    let warn = findings.iter().filter(|f| f.severity == Severity::Warn).count();
    let good = findings.iter().filter(|f| f.severity == Severity::Good).count();

    let fixing = app.health.fix_job.is_some();
    let mut fix: Option<Fix> = None;
    card(ui, |ui| {
        split_row(
            ui,
            "findings-head",
            260.0,
            |ui| {
                let issues = bad + warn;
                section(ui, &if issues == 0 { "No problems found".to_string() } else { format!("{issues} thing{} worth fixing", if issues == 1 { "" } else { "s" }) });
            },
            |ui| {
                if good > 0 {
                    chip_icon(ui, Some(icon::CHECK), format!("{good} fine"), Tone::Neutral);
                }
                if warn > 0 {
                    chip(ui, format!("{warn} warning{}", if warn == 1 { "" } else { "s" }), Tone::Warn);
                }
                if bad > 0 {
                    chip(ui, format!("{bad} problem{}", if bad == 1 { "" } else { "s" }), Tone::Bad);
                }
            },
        );
        for f in &findings {
            ui.add_space(10.0);
            divider(ui);
            ui.add_space(10.0);
            let (tone, ic) = sev(f.severity);
            let label = match &f.fix {
                Fix::None => None,
                Fix::Tweak(_) | Fix::RefreshRate { .. } => Some("Fix it"),
                Fix::OpenStartup => Some("Review"),
                Fix::OpenSettings => Some("Open settings"),
            };
            split_row(
                ui,
                &f.title,
                if label.is_some() { 130.0 } else { 0.0 },
                |ui| {
                    ui.horizontal_top(|ui| {
                        icon_badge(ui, ic, tone, 30.0);
                        ui.add_space(4.0);
                        ui.vertical(|ui| {
                            ui.spacing_mut().item_spacing.y = 2.0;
                            ui.add(egui::Label::new(tw(&f.title, 14.0, W::Semi)).wrap());
                            ui.add(egui::Label::new(small(&f.detail)).wrap());
                        });
                    });
                },
                |ui| {
                    if let Some(l) = label {
                        let kind = if matches!(f.severity, Severity::Bad | Severity::Warn) { Kind::Secondary } else { Kind::Outline };
                        if Btn::new(kind, l).enabled(!fixing).show(ui).clicked() {
                            fix = Some(f.fix.clone());
                        }
                    }
                },
            );
        }
    });

    match fix {
        Some(Fix::Tweak(id)) => {
            let Some(t) = catalog::find(id) else { return };
            if !app.elevated {
                app.toast(ToastKind::Err, "Needs admin rights");
                return;
            }
            let run = move |app: &mut App| {
                let ctx = app.monitor_ctx();
                app.health.fix_job = Some(Job::spawn(&ctx, move || tweaks::apply(t).map(|_| format!("Applied: {}", t.title))));
            };
            if t.risk == tweaks::Risk::High {
                app.ask("Advanced tweak", &format!("{}\n\n{}", t.title, t.description), "Apply anyway", true, run);
            } else {
                run(app);
            }
        }
        Some(Fix::RefreshRate { device, hz }) => {
            app.ask(
                "Change refresh rate?",
                &format!("Switch {device} to {hz} Hz? If the screen goes black, it will not come back on its own — wait 15 seconds and press Win+Ctrl+Shift+B, or change it back in Display settings."),
                "Switch",
                false,
                move |app| {
                    let ctx = app.monitor_ctx();
                    app.health.fix_job = Some(Job::spawn(&ctx, move || {
                        display::set_refresh_rate(&device, hz).map(|_| format!("{device} now runs at {hz} Hz"))
                    }));
                },
            );
        }
        Some(Fix::OpenStartup) => {
            app.go(Page::Debloat);
            app.debloat.tab = super::debloat::Tab::Startup;
        }
        Some(Fix::OpenSettings) => app.go(Page::Settings),
        Some(Fix::None) | None => {}
    }
}
