use super::data::{lock, take, Job};
use super::theme::*;
use super::{App, Page, ToastKind};
use crate::health::{self, Finding, Fix, Severity};
use crate::sys::{display, net};
use crate::tweaks::{self, catalog};
use eframe::egui::{self, Align, Layout, RichText, Ui};

#[derive(Default)]
pub struct State {
    findings: Option<Vec<Finding>>,
    adapters: Vec<net::AdapterInfo>,
    job: Option<Job<(Vec<Finding>, Vec<net::AdapterInfo>)>>,
    fix_job: Option<Job<Result<String, String>>>,
}

fn sev_color(s: Severity) -> egui::Color32 {
    match s {
        Severity::Bad => BAD,
        Severity::Warn => WARN,
        Severity::Info => INFO,
        Severity::Good => GOOD,
    }
}

fn scan(app: &mut App, ctx: &egui::Context) {
    app.health.job = Some(Job::spawn(ctx, || (health::scan(), net::active_adapters())));
}

pub fn show(app: &mut App, ui: &mut Ui) {
    let ctx = ui.ctx().clone();
    page_header(ui, "Health check", "Finds the usual causes of stutter, input lag and network lag on this PC.");

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
    ui.horizontal(|ui| {
        if ui.add_enabled(!scanning, egui::Button::new("Scan again")).clicked() {
            scan(app, &ctx);
        }
        if scanning {
            ui.spinner();
            ui.label(muted("Scanning..."));
        }
    });
    ui.add_space(8.0);

    // Network summary (live)
    let (stats, target, ping) = {
        let m = lock(&app.monitor.metrics);
        (m.ping_stats(), m.ping_target.clone(), m.ping_ms)
    };
    card(ui, |ui| {
        section(ui, "Connection");
        ui.horizontal_wrapped(|ui| {
            for a in &app.health.adapters {
                chip(
                    ui,
                    format!("{} - {} - {} Mbps", a.name, if a.wifi { "Wi-Fi" } else { "Wired" }, a.speed_mbps),
                    if a.wifi || (a.speed_mbps > 0 && a.speed_mbps < 1000) { WARN } else { GOOD },
                );
            }
        });
        ui.label(format!(
            "Ping to {}: {}   avg {:.0} ms   jitter {:.1} ms   packet loss {:.1}%   ({} samples)",
            if target.is_empty() { "-" } else { &target },
            ping.map(|p| format!("{p} ms")).unwrap_or_else(|| "-".into()),
            stats.avg,
            stats.jitter,
            stats.loss,
            stats.samples
        ));
        let verdict = if stats.samples < 10 {
            ("Collecting samples...", MUTED)
        } else if stats.loss > 2.0 {
            ("Packet loss detected - check Wi-Fi signal / cable / router, or contact your ISP.", BAD)
        } else if stats.jitter > 10.0 {
            ("High jitter - something on your network (downloads, streaming, Wi-Fi) is competing for bandwidth.", WARN)
        } else {
            ("Stable connection.", GOOD)
        };
        ui.label(RichText::new(verdict.0).color(verdict.1));
    });
    ui.add_space(8.0);

    let Some(findings) = app.health.findings.clone() else { return };
    let issues = findings.iter().filter(|f| matches!(f.severity, Severity::Bad | Severity::Warn)).count();
    ui.label(RichText::new(if issues == 0 { "No problems found.".to_string() } else { format!("{issues} thing(s) worth fixing") }).strong().size(16.0));
    ui.add_space(4.0);

    let fixing = app.health.fix_job.is_some();
    let mut fix: Option<Fix> = None;
    for f in &findings {
        let c = sev_color(f.severity);
        card_tinted(ui, c, |ui| {
            ui.horizontal(|ui| {
                status_dot(ui, c, 5.0);
                ui.vertical(|ui| {
                    ui.set_max_width(ui.available_width() - 150.0);
                    ui.label(RichText::new(&f.title).strong());
                    ui.label(small(&f.detail));
                });
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    let label = match &f.fix {
                        Fix::None => None,
                        Fix::Tweak(_) | Fix::RefreshRate { .. } => Some("Fix it"),
                        Fix::OpenStartup => Some("Review"),
                        Fix::OpenSettings => Some("Open settings"),
                    };
                    if let Some(l) = label {
                        if ui.add_enabled(!fixing, egui::Button::new(l)).clicked() {
                            fix = Some(f.fix.clone());
                        }
                    }
                });
            });
        });
        ui.add_space(4.0);
    }

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
                &format!("Switch {device} to {hz} Hz? If the screen goes black, it will not come back on its own - wait 15 seconds and press Win+Ctrl+Shift+B, or change it back in Display settings."),
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
            app.page = Page::Debloat;
            app.debloat.tab = super::debloat::Tab::Startup;
        }
        Some(Fix::OpenSettings) => app.page = Page::Settings,
        Some(Fix::None) | None => {}
    }
}
