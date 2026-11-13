# Avix

**A tiny, real-time performance optimizer and debloater for Windows 10/11 — built in Rust.**

Avix sits in your tray using ~3 MB of RAM. The moment a game, stream or local AI model starts, it
switches your PC into a tuned "boost" state, keeps it there while you play, and puts **everything**
back exactly as it was when you're done.

| | Measured on a Ryzen 7 5700X / RTX 3060 / 16 GB |
|---|---|
| Engine (tray, always on) | **~2.5 MB** private RAM, **~0.02 %** CPU |
| Dashboard (only while open) | ~53 MB RAM (mostly the GPU driver), ~0.06 % CPU — fully freed when closed |
| Binary | single 4.9 MB `Avix.exe`, no runtime, no installer |

![Dashboard while boosting](docs/screenshots/dashboard-boosting.png)
![Debloat & tweaks](docs/screenshots/debloat.png)

---

## What the boost does

When an app from a profile starts (or *any* app goes fullscreen), Avix:

1. **Switches power plan** to *Avix Ultimate Performance* — a copy of Windows' hidden Ultimate plan,
   tuned for latency: USB selective suspend **off** (no sleepy mice/headsets), PCIe link power saving
   off, cores never parked, CPU min state 100 %, no disk/system sleep. Your previous plan comes back after.
2. **Raises your game** to High priority and opts it out of EcoQoS.
3. **Throttles everything else in your session** — Below-normal CPU priority, *Efficiency mode* (EcoQoS),
   lower memory priority and **low disk I/O priority** — so background apps stop stealing CPU time and
   disk bandwidth from your game. The window you're looking at is always exempt (alt-tab stays snappy).
4. **Pauses Windows Update, Update Orchestrator, Delivery Optimization and BITS**, so nothing downloads,
   installs or seeds updates mid-match (the #1 hidden cause of random network lag), and restarts them after.
5. **Frees memory when RAM runs low**: it cleans the standby list (like ISLC), and at 80 % RAM use it
   makes throttled background apps hand their memory back. This makes a big difference on 8 GB PCs.
6. **Laptops with two GPUs:** it sets Windows' graphics preference so each boosted game runs on the
   dedicated GPU from its next launch. Choices you made yourself are never overridden.
7. Optionally: requests 0.5 ms timer resolution, closes apps you pick, keeps companions (e.g. OBS) responsive.

Everything is applied to newly started apps too, and restored *exactly* afterwards (original priority,
EcoQoS state, memory and I/O priority — per process). A crash-safe journal means even if Avix is killed
mid-boost, the next start undoes it.

### Anti-cheat safety (VALORANT, LoL, Fortnite, Apex, CoD …)

Avix **never** injects, reads or writes game memory, and never installs drivers. For games with kernel
anti-cheat (Vanguard, EAC, BattlEye, Ricochet…) Avix doesn't even open a handle to the game process — it
optimizes everything *around* the game. To raise a protected game's own priority, Avix uses Windows'
built-in *launch priority* (Image File Execution Options `CpuPriorityClass`), which Windows applies when
the game starts. Anti-cheat services, voice chat, audio and peripheral software (G HUB, Synapse, iCUE…)
are on a **never touch** list.

### Profiles (all editable)

| Profile | Triggers (examples) | Notes |
|---|---|---|
| **Gaming** | VALORANT, CS2, Fortnite, Apex, LoL, Overwatch, R6, CoD, GTA V, Rocket League, Dota 2, Marvel Rivals… | High priority, standby cleaner, OBS kept responsive |
| **Streaming** | OBS, Streamlabs, XSplit | Above-normal priority for the encoder |
| **AI / Compute** | Ollama, llama.cpp, LM Studio, KoboldCpp, Jan, GPT4All, ComfyUI, Blender | Only boosts while the app is actually working (CPU threshold) |

Plus **auto-detect** for any game that isn't in a profile — no list to keep up to date, and it works
whether the game started before or after Avix, fullscreen or windowed. A game counts if:
- **Windows recognises it** — the list Xbox Game Bar and Game Mode use (matched by full exe path), or
- **it runs from a game store's library** with a real window — Steam, Epic, GOG, Ubisoft, EA, Xbox,
  itch.io, Amazon Games, or any folder named `Games` (launchers, overlays, installers and crash handlers
  are skipped), or
- **it goes fullscreen**, even if it isn't the window in focus (browsers and video players excluded), or
- **it's the app you're using and keeps the GPU's 3D engine busy** (≥ 20 % for a few seconds) — this catches
  games from anywhere. It stops counting after 2 minutes idle; the GPU counters are only opened while checking.

Detected games use the Gaming profile; the dashboard offers to add the GPU/fullscreen ones to a profile.
Each source can be switched off in **Settings → Auto-detect games**.

---

## Debloat & tweaks (every change is reversible)

Before touching anything, Avix records the original value (`tweak_backup.json`). Each tweak has
**Apply / Revert**, a risk label and an honest description. There's also a *Create restore point* button.

- **Gaming:** Game DVR background recording off, Game Mode on, mouse acceleration off, MMCSS game priority,
  optimizations for windowed games (lower latency in borderless mode), VALORANT launch priority,
  *(advanced)* exclusive-fullscreen preference, MPO off, global timer requests, Memory Integrity (HVCI)
  off — clearly marked as a security trade-off.
- **Performance:** foreground CPU reservation, Edge background/startup boost off, Fast Startup off,
  window animations off…
- **Network:** multimedia network throttling off, **stop Windows uploading updates to strangers (P2P)**, Nagle off for TCP games.
- **Privacy & AI:** telemetry minimum + services + scheduled tasks (incl. CompatTelRunner CPU spikes),
  Copilot off, Recall off, advertising ID, activity history, Bing in Start search…
- **Ads & nags:** silently installed apps, Start/lock-screen ads, "finish setting up your device", feedback prompts.
- **Services:** Maps, Retail Demo, Fax; *(optional)* SysMain, Search indexer, Print Spooler.
- **Preinstalled apps:** scans what's actually installed and removes Store bloat (incl. Widgets) for all users
  (and stops it being re-provisioned). Things games/Windows need (Xbox Identity Provider, Store, frameworks…) are never listed.
- **OneDrive** uninstall, **Startup apps** manager (same switch Task Manager uses).

## Health check

Finds the usual suspects behind stutter, input lag and "network lag": monitor running below its max
refresh rate (one-click fix), Wi-Fi instead of Ethernet, **Ethernet linked at 100 Mbps (bad cable)**,
Game DVR, Game Mode, mouse acceleration, HVCI, missing page file, low disk space, pending update reboot,
too many startup apps — plus a live ping / jitter / packet-loss monitor.

**Hardware set-up:** the problems that cost the most FPS, especially on budget PCs, which Avix can spot
but not change for you:
- **RAM running below its rated speed** (XMP / EXPO off in the BIOS), read from the firmware and the
  sticks' part numbers
- **only one RAM stick, or both in the same channel**, so it runs single-channel
- **old graphics drivers**, or no graphics driver at all
- **games installed on a hard drive**
- **a laptop on battery or in battery saver**
- **two GPUs**, with the advice to keep games on the dedicated one

---

## Download

Grab **`Avix.exe`** from the [Releases page](../../releases) — no installer, no runtime.

> Early preview (v0.1.x). The exe isn't code-signed yet, so Windows SmartScreen may say
> "Windows protected your PC" — click **More info → Run anyway**. See the [code signing policy](#code-signing-policy).

**Privacy:** no telemetry, no accounts. The update check (opt-in) and the dashboard's ping monitor are
the only network traffic, and both can be turned off: [PRIVACY.md](PRIVACY.md).

## Using it

1. Put `Avix.exe` somewhere permanent, e.g. `%LOCALAPPDATA%\Programs\Avix\Avix.exe`.
2. Double-click it → accept the UAC prompt (admin is needed for services, power plans, tweaks).
   The tray icon appears (purple = watching, green = boosting, grey = paused) and the dashboard opens.
3. **Settings → Start Avix with Windows** (uses a Task Scheduler logon task, so no UAC prompt every boot).
4. Recommended first steps: **Debloat & tweaks → Apply all recommended**, then **Health check**, then reboot once.

Tray menu: open dashboard · Auto-boost · Boost now (per profile) · Pause · free standby memory · Exit
(Exit restores everything). **Ctrl+Alt+B** toggles *Boost now*.

**Updates:** once a day Avix asks GitHub for the latest release (a single HTTPS request; nothing about
your PC is sent). When there's a new version, the tray shows a one-time notification (never mid-game), the
tray menu gets *Update available*, and the dashboard shows a **Download** button. To update: download the
new `Avix.exe`, exit Avix from the tray, and replace the old file. Your settings stay. You can turn
this off, or check right away, in **Settings → Updates**.

### Command line

```
Avix.exe              start engine (if needed) + open dashboard
Avix.exe --tray       engine only (what autostart uses)
Avix.exe --paused     start with auto-boost off
Avix.exe --restore    undo anything left by an interrupted boost
Avix.exe --diag       read-only self-test -> %LOCALAPPDATA%\Avix\avix-diag.txt
Avix.exe --no-elevate run without the UAC prompt (limited mode)
```

Data lives in `%LOCALAPPDATA%\Avix\`: `config.json`, logs, `tweak_backup.json`, `boost-journal.json`,
`update.json` (last update check).

### Uninstall

Settings → turn off *Start with Windows* → *Remove Avix power plan* → (optionally) Revert tweaks → Exit
from the tray → delete `Avix.exe` and `%LOCALAPPDATA%\Avix`.

---

## Honest limits

- Avix can't fix lag caused by your ISP, Wi-Fi interference, the game server, or a GPU that's simply maxed out.
- VALORANT/CS2 use UDP, so "Nagle off" won't change their ping (it's labelled as such).
- On Windows 11 a 0.5 ms timer request only reaches games if *Allow global timer-resolution requests* is applied.
- Some services (e.g. Update Orchestrator) may refuse to pause on some builds — Avix logs it and carries on.

---

## Building from source

Requirements: Rust (stable). Then:

```powershell
powershell -ExecutionPolicy Bypass -File .\build.ps1   # -> dist\Avix.exe
```

Notes:
- It builds with rustup's **GNU** toolchain (`x86_64-pc-windows-gnu`, no Visual Studio needed).
  `Cargo.lock` pins a few crates (`parking_lot`, `libloading`, `arboard`, `webbrowser`) to releases that
  don't use `raw-dylib`, because that needs a full MinGW `dlltool` + assembler. Build with `--locked` and
  don't `cargo update` those unless you switch to the MSVC toolchain (Visual Studio Build Tools), where the
  pins can be dropped. `build.rs` also provides an empty `libshlwapi.a` stub for the same reason.
- Code layout:
  - `src/engine/` – boost state machine, tray + message loop, crash-safe journal
  - `src/sys/` – Win32/NT wrappers (process snapshot, priorities/EcoQoS/IO priority, power plans, services,
    registry, memory lists, GPU counters, ICMP ping, displays)
  - `src/tweaks/` – tweak catalogue with backup/revert, Store-app removal, startup apps, scheduled tasks
  - `src/ui/` – egui dashboard (only runs while open)
  - `src/ipc.rs` – engine ⇄ dashboard named pipe (local only, owner-only writes)

## Testing

`avix --diag` is a read-only report. Debug builds also have an **admin integration test**
(`cargo build`, then run `target\debug\avix.exe --selftest` elevated; the report lands in the data
folder). It round-trips every system-changing feature — restore point, power plan create/tune/switch,
standby purge, timer resolution, process priority / EcoQoS / memory / I/O priority, launch priority,
startup toggles, the autostart task, the app-removal script and every not-yet-applied tweak — and checks
the PC is byte-for-byte back where it started (already-applied tweaks are skipped). Set
`AVIX_SELFTEST_ONLY=timer,power,...` to run a subset and `AVIX_DATA_DIR` to keep test data separate.
In debug builds `AVIX_INSTANCE=name` runs a separate engine (own pipe and single-instance lock), so tests
can run next to a real Avix.
v0.1.1 passed all 67 self-test checks plus a 25-check elevated engine run (boost, exact restore,
hard-kill crash recovery).

## Code signing policy

Free code signing provided by [SignPath.io](https://about.signpath.io), certificate by
[SignPath Foundation](https://signpath.org).

*Avix is applying to the SignPath Foundation. Until it is approved, releases are unsigned.*

- **Committers and reviewers:** [Avilash Ghosh](https://github.com/Avilash2001)
- **Approvers:** [Avilash Ghosh](https://github.com/Avilash2001)

Releases are built from this repository by GitHub Actions, and only those builds are signed. Every
signing request is approved by hand. The privacy policy is in [PRIVACY.md](PRIVACY.md): Avix sends
nothing about you or your PC, and its two kinds of network requests are opt-in or can be turned off.

## License

[MIT](LICENSE) © 2026 Avilash Ghosh

## Releasing

GitHub Actions ([`.github/workflows/build.yml`](.github/workflows/build.yml)) tests and builds every push
and pull request on Windows. It links with rustup's bundled MinGW (`-C link-self-contained=yes`) and the
same `--locked` pins as `build.ps1`, so CI builds match a PC that has only rustup. Each run's *Artifacts*
has the exe. To ship a version:

1. Bump `version` in `Cargo.toml`. Run a plain `cargo build` once, which updates only Avix's own entry
   in `Cargo.lock`. Commit and push.
2. Tag it with the same version: `git tag v0.1.4` then `git push origin v0.1.4`. CI fails if the tag and
   `Cargo.toml` disagree.
3. CI builds from scratch (no cache for tags), runs the tests, and creates a **draft** release with
   `Avix.exe` and `Avix.exe.sha256` attached.
4. Write the notes and publish. Avix's update check only sees published, non-pre-release releases, so
   nobody is told about a version before its exe is there.

If a release for the tag already exists, CI replaces its `Avix.exe` with the CI build.

## Before sharing it publicly

### Code signing (removes "Windows protected your PC")

Windows SmartScreen warns about any unsigned download it hasn't seen many times. No code change can fix
that; a code-signing certificate is the real fix. Because Avix is built by public CI, the
[SignPath Foundation](https://signpath.org) can sign it for free:

1. **Apply** at [signpath.org/apply](https://signpath.org/apply). They sign OSI-licensed (MIT is fine),
   maintained, already-released projects with no proprietary code. They want:
   - a *Code signing policy* on the project page (see [Code signing policy](#code-signing-policy)),
   - a privacy policy ([PRIVACY.md](PRIVACY.md)),
   - MFA on GitHub and SignPath for everyone on the team.

   Signed files show **SignPath Foundation** as the publisher.
2. **Metadata:** the Foundation enforces product name/version metadata. `Avix.exe` carries a version
   resource (product name `Avix`, version = `Cargo.toml`), written by `build.rs` together with the
   manifest and the icon. Use those values in SignPath's artifact configuration.
3. **Once accepted**, in SignPath:
   - Link the predefined *GitHub.com* trusted build system to the project.
   - Add a CI user as *Submitter* of the release signing policy.
   - Use an artifact configuration with a root `<pe-file path="Avix.exe">` and `<authenticode-sign/>`.
     The workflow uploads the exe unzipped.
4. **Repository secrets** (Settings → Secrets and variables → Actions):
   - `SIGNPATH_API_TOKEN`: the CI user's API token
   - `SIGNPATH_ORGANIZATION_ID`

   Set the project and signing-policy slugs in `build.yml`, then uncomment its *Sign with SignPath* and
   *Use the signed Avix.exe* steps. Each release then waits until an approver approves the signing
   request in SignPath.

A paid individual certificate (e.g. Certum's open-source one) works in the same place.

Even a signed exe can still show SmartScreen for a while. Windows builds reputation per file and
certificate as people download and run it, so the first downloads of a new release may still see the
warning. It names the publisher instead of "Unknown publisher" and stops much sooner.

### Also worth doing
- A one-click updater (download, verify the SHA-256, swap the exe, restart).
- Test on Windows 10, and expand per-game defaults.
