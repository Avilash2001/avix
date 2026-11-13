# Avix privacy policy

**Short version:** Avix has no telemetry, no accounts and no ads. It never sends anything about you
or your PC anywhere. It makes exactly two kinds of network requests, both described below, and you
can turn both off.

## What goes over the network

Avix makes no network requests until you've answered the first-run screen.

1. **Update check: only if you allow it** (first-run screen, or **Settings → Updates**).
   - **When:** once a day, at most.
   - **What:** a single HTTPS request to `api.github.com/repos/Avilash2001/avix/releases/latest`
     to see whether a newer version exists.
   - **Contents:** only what any web request carries, i.e. your IP address and the user agent
     `Avix/<version>`. There's no identifier, and nothing about your hardware, games or settings.
     GitHub's [privacy statement](https://docs.github.com/site-policy/privacy-policies/github-general-privacy-statement)
     covers that request.
   - **Turning it off:** Settings → Updates.
2. **Latency monitor: only while the dashboard window is open.**
   - **What:** once a second, Avix sends a standard ICMP echo ("ping") to the ping target and
     shows the round-trip time. The default target is `1.1.1.1`, Cloudflare's public DNS.
   - **Contents:** the packet holds no data about you.
   - **Turning it off:** clear the target in Settings → Latency monitor, or set it to a server of
     your choice.

Links you click in the app ("Download", "Privacy policy") open in your browser.

## What stays on your PC

Avix keeps its files in `%LOCALAPPDATA%\Avix`, and they never leave your PC:

- **`config.json`:** your settings and profiles.
- **`avix-*.log`:** what Avix did and when, e.g. which apps it adjusted during a boost.
- **`tweak_backup.json`, `boost-journal.json`, `gpu_prefs.json`:** the original values Avix
  restores when you revert a tweak, end a boost or turn a feature off.
- **`update.json`:** the result of the last update check.

To remove all of it, delete that folder. The README's *Uninstall* section has the full steps.

## Changes Avix makes to your system

Avix changes system settings, and every change can be undone. The README lists them all:

- **While a game, stream or AI app runs:** it switches the power plan, adjusts process priorities,
  pauses update services and frees memory. Everything is restored afterwards.
- **Only when you click:** tweaks and app removal.

The first-run screen summarises this too.

## Contact

Questions or concerns: open an issue at <https://github.com/Avilash2001/avix/issues>.
If this policy changes, the change will be listed in the release notes.
