# Contributing to Avix

Thanks for helping. Avix is a small project run by one developer, so every bug report, test on a
different PC and pull request makes a real difference.

## Ways to help without writing code

- **Report a bug.** Open an [issue](../../issues/new/choose) and say what you expected, what
  happened, your Avix version (Settings → About) and your Windows version (`winver`).
  `Avix.exe --diag` writes a read-only report to `%LOCALAPPDATA%\Avix\avix-diag.txt`, and the
  logs are in the same folder (`avix-engine.log`, `avix-ui.log`). They help a lot, but they list
  running apps, installed games and paths that can include your Windows user name, so read them
  before attaching.
- **A game Avix doesn't detect or boost?** Open an issue with the game's exe name, where it's
  installed from (Steam, Epic, …) and whether it uses kernel anti-cheat.
- **Test on hardware the author doesn't have**: Windows 10, laptops with two GPUs, Intel CPUs,
  AMD graphics, 8 GB PCs. Even "it worked" is useful.
- **Star the repo** and tell a friend. It's how other people find Avix.

## Building

Avix builds with rustup's GNU toolchain; no Visual Studio needed.

```powershell
rustup default stable-x86_64-pc-windows-gnu
powershell -ExecutionPolicy Bypass -File .\build.ps1   # -> dist\Avix.exe
```

Always build with `--locked` and don't `cargo update`: `Cargo.lock` pins a few crates so the GNU
toolchain can link them. The README's *Toolchain notes and code layout* explains why, and where
each part of the code lives.

### Running a test build next to your own Avix

Debug builds can run as a separate instance, so they never touch the Avix you use every day:

```powershell
cargo build --locked
$env:AVIX_INSTANCE = "dev"                      # own pipe and single-instance lock
$env:AVIX_DATA_DIR = "$env:TEMP\avix-dev"       # own config, logs and backups
.\target\debug\avix.exe --no-elevate
```

## Before you open a pull request

- `cargo test --locked --bin avix` passes. GitHub Actions runs the same build and tests on every
  pull request (the *Check* workflow) and shows the result on the PR. On a first pull request from
  a fork, it starts once the maintainer approves the run.
- If you changed anything that touches Windows (priorities, services, power plans, tweaks), run
  the admin self-test: `target\debug\avix.exe --selftest` from an elevated prompt, then read
  `avix-selftest.txt` in the data folder. It checks the PC ends up exactly where it started.
- If you changed the dashboard, add before/after screenshots. `AVIX_SHOT=<folder>` renders every
  page in light and dark (see `src/ui/shot.rs`).

## Ground rules

These are what make people trust Avix enough to leave it running:

1. **Every change is reversible.** Record the original value before changing anything, and
   restore it exactly afterwards.
2. **Never touch game memory, inject into processes or install drivers.** Games with kernel
   anti-cheat don't even get a process handle.
3. **No telemetry.** Avix sends nothing about the user or their PC.
4. **Honest copy.** Each tweak says plainly what it does and what it risks. No inflated claims.
5. **Stay light.** The engine is a few MB of RAM, and the dashboard doesn't repaint while nothing
   changes.

Keep pull requests small and focused, and say in the description what you tested and on which
Windows version. For bigger changes, open an issue first so we can agree on the approach.

## License

By contributing, you agree that your contributions are licensed under the [MIT License](LICENSE).
