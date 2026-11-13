# Product

<!-- impeccable:product-schema 1 -->

## Platform

windows

Native Windows 10/11 desktop app, not one of web / ios / android / adaptive. The dashboard is an
egui window (eframe 0.31, glow renderer); there is no HTML or CSS anywhere in the product.

## Users

All three groups below, with no single primary (the project started recently and its audience is
still forming):

- **Gamers doing quick check-ins.** Open the dashboard for a minute to confirm the boost is working,
  apply tweaks once, then close it and play.
- **Budget-PC owners who aren't technical.** Often friends of the author with weak PCs. They need
  the app to feel safe, plain and trustworthy before they let it change Windows.
- **Tinkerers who live in it.** Scan the process table, edit boost profiles, safety lists and
  thresholds in detail.

## Product Purpose

Avix makes a Windows PC faster while a game, stream or local AI model is running, and leaves the PC
exactly as it was afterwards. It also debloats Windows with tweaks that can each be reverted, and
runs a health check for the usual causes of stutter, input lag and network lag. Success: the user
trusts it enough to leave it running in the tray and feels the difference in-game.

## Positioning

A tiny tray engine (~2.5 MB RAM, ~0.02 % CPU) switches the PC into a tuned "boost" state the moment
a game starts and restores every change exactly when it ends, per process, with a crash-safe
journal. It never injects into games, reads game memory or installs drivers, so it is safe beside
kernel anti-cheat. Every debloat tweak records the original value first and has its own Revert.

## Operating Context

- The engine lives in the notification area (tray icon with Idle / Boosting / Paused states).
  The dashboard is a separate process spawned on demand and fully exited when closed, so it
  costs nothing while the user games.
- The dashboard is opened in short sessions, often right before or after playing, sometimes
  while a game runs on another monitor.
- Single portable `Avix.exe`, no installer, no runtime. Distributed through GitHub Releases;
  not code-signed yet.

## Capabilities and Constraints

- Pages: Dashboard, Boost profiles, Processes, Debloat & tweaks, Health check, Settings; plus a
  first-run welcome, confirm dialogs, toasts and an unsaved-changes bar.
- Binary budget: the exe may grow by up to about 1 MB for embedded fonts (confirmed). Everything
  else stays in the single exe.
- The dashboard's RAM and CPU use should stay small (it is a performance tool); continuous
  repainting must not run when nothing is changing.
- The app icon is drawn in code (`src/icon_render.rs`) and shared by the window, the tray (16 px
  and up, three states) and the exe's embedded icon resource (`build.rs`).
- Requires admin for most changes; a limited mode runs without it.

## Brand Commitments

- The name **Avix** is the only binding brand element. The previous lightning bolt, purple accent
  and tile shape are open to replacement (confirmed).
- Honest, plain-spoken copy: every tweak states its risk and what it really does; nothing is
  oversold.

## Evidence on Hand

- Measured footprint on a Ryzen 7 5700X / RTX 3060 / 16 GB (README): engine ~2.5 MB RAM,
  ~0.02 % CPU; dashboard ~53 MB RAM while open; 4.9 MB exe.
- Screenshots in `docs/screenshots/` (to be replaced by the redesign).
- No testimonials, user counts, benchmarks of FPS gains or press exist. Do not invent any.

## Product Principles

1. Trust before speed: show exactly what Avix changed and that it can be undone.
2. Glanceable first, detailed on demand: the boost state reads in a second; depth is one step away.
3. Never cost the game anything: the UI is light, quiet and absent when closed.
4. Plain words over jargon, with the technical name available for those who want it.
