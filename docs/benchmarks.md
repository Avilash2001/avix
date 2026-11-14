# Benchmarking Avix

How to measure whether Avix's boost makes a difference on your PC, in a way other people can trust.
Results are welcome as a [benchmark report](https://github.com/Avilash2001/avix/issues/new?template=benchmark_report.yml),
including results that show no difference.

## What to expect

The boost helps most when **other programs compete with the game** for CPU time, disk or memory:
CPUs with 4–6 threads, 8 GB of RAM, a browser and Discord open, Windows Update downloading. On a
fast PC with nothing running in the background, expect little or no difference. That's a valid
result too.

This guide measures the **boost only** (power plan, priorities, paused update services, memory).
Debloat tweaks are permanent changes and are measured separately: apply or revert them before you
start, and keep them the same for every run.

## What you need

- [CapFrameX](https://www.capframex.com) (free, open source; it uses Intel's PresentMon to record
  every frame).
- A game with a **built-in benchmark** (most repeatable: e.g. Shadow of the Tomb Raider,
  Cyberpunk 2077, F1, Forza Horizon, Total War), or a fixed route you can repeat closely (CS2
  workshop benchmark maps, the VALORANT Range).
- About 45 minutes.

## Steps

1. **Prepare the same conditions for every run.** Restart the PC, open your usual background apps
   and leave them open for the whole test. A realistic set: a browser with a few tabs and a 1080p
   YouTube video playing, Discord, the game's launcher. Write down exactly what was open.
2. **Fix the game's settings**: resolution, preset, V-Sync **off**, frame cap **off** (a cap hides
   differences).
3. **Set CapFrameX** to capture 60 seconds (Capture tab → capture time) and pick a hotkey.
4. **Alternate six runs**: Avix **paused** (tray → Pause), Avix **on** (tray → Auto-boost; check
   the tray icon turns green when the game starts), paused, on, paused, on. Alternating cancels out
   the PC warming up over time. Start the capture at the same moment of the benchmark each time.
5. **Read the results** in CapFrameX's Analysis tab: **Average FPS**, **1% low** and **0.1% low**
   for each run. Use the Comparison tab for a chart of a paused run against an on run.
6. **Report the median** of the three runs for each mode, plus the lowest and highest. If the
   difference between modes is smaller than the spread between runs of the same mode, call it
   "no measurable difference".

## Results template

```
PC:          CPU / GPU / RAM (size, speed, sticks) / drive the game is on
Windows:     11 24H2 (from winver)
Avix:        0.1.x
Game:        name, settings preset, resolution, scene (built-in benchmark / route)
Background:  what was open

| Avix   | Run | Average FPS | 1% low | 0.1% low |
|--------|-----|-------------|--------|----------|
| paused | 1   |             |        |          |
| on     | 1   |             |        |          |
| paused | 2   |             |        |          |
| on     | 2   |             |        |          |
| paused | 3   |             |        |          |
| on     | 3   |             |        |          |

Median, paused:  avg ___  1% ___  0.1% ___
Median, on:      avg ___  1% ___  0.1% ___
```

Attach the CapFrameX comparison chart if you can.
