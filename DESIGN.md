---
name: Avix
description: A calm, airy Windows performance dashboard with one electric-blue voice, drawn natively in egui.
colors:
  canvas: "#FFFFFF"
  surface: "#FFFFFF"
  sunken: "#F3F3F6"
  hover: "#EBEBF0"
  border: "#E7E7ED"
  border-strong: "#D6D6DF"
  ink: "#0B0B10"
  muted: "#5A5D68"
  faint: "#707481"
  electric-blue: "#3D3DF5"
  electric-blue-hover: "#2C2CE0"
  electric-blue-soft: "#EDEDFE"
  electric-blue-text: "#3232E4"
  selection: "#CDCDFC"
  boost-green: "#16A34A"
  boost-green-text: "#12804A"
  warn-amber: "#F59E0B"
  warn-amber-text: "#A9520A"
  bad-red: "#EF4444"
  bad-red-text: "#D12A2A"
  toggle-off: "#E6E6EC"
  toggle-ring: "#8E8E9C"
  on-accent: "#FFFFFF"
  dark-canvas: "#0B0B0E"
  dark-surface: "#131317"
  dark-sunken: "#1B1B21"
  dark-hover: "#23232A"
  dark-border: "#25252D"
  dark-border-strong: "#34343F"
  dark-ink: "#EDEDF1"
  dark-muted: "#A3A6B1"
  dark-faint: "#888B98"
  dark-electric-blue: "#5C5CFF"
  dark-electric-blue-hover: "#6F6FFF"
  dark-electric-blue-soft: "#1E1E44"
  dark-electric-blue-text: "#A3A3FF"
  dark-selection: "#30307C"
  dark-boost-green: "#2FBF71"
  dark-boost-green-text: "#4ADE80"
  dark-warn-amber: "#F5A524"
  dark-warn-amber-text: "#FBBF24"
  dark-bad-red: "#F2555A"
  dark-bad-red-text: "#FF7D81"
  dark-toggle-off: "#2C2C35"
  dark-toggle-ring: "#72727F"
typography:
  display:
    fontFamily: "Geist, Segoe UI, sans-serif"
    fontSize: "26px"
    fontWeight: 600
    letterSpacing: "-0.5px"
    fontFeature: "tnum"
  headline:
    fontFamily: "Geist, Segoe UI, sans-serif"
    fontSize: "22px"
    fontWeight: 600
    letterSpacing: "-0.4px"
  metric:
    fontFamily: "Geist, Segoe UI, sans-serif"
    fontSize: "24px"
    fontWeight: 600
    fontFeature: "tnum"
  title:
    fontFamily: "Geist, Segoe UI, sans-serif"
    fontSize: "15px"
    fontWeight: 600
    letterSpacing: "-0.15px"
  body:
    fontFamily: "Geist, Segoe UI, sans-serif"
    fontSize: "13.5px"
    fontWeight: 400
    fontFeature: "tnum"
  body-strong:
    fontFamily: "Geist, Segoe UI, sans-serif"
    fontSize: "13.5px"
    fontWeight: 500
  label:
    fontFamily: "Geist, Segoe UI, sans-serif"
    fontSize: "12px"
    fontWeight: 500
  small:
    fontFamily: "Geist, Segoe UI, sans-serif"
    fontSize: "12px"
    fontWeight: 400
  chip:
    fontFamily: "Geist, Segoe UI, sans-serif"
    fontSize: "11.5px"
    fontWeight: 500
  caption:
    fontFamily: "Geist, Segoe UI, sans-serif"
    fontSize: "11px"
    fontWeight: 400
  wordmark:
    fontFamily: "Geist, Segoe UI, sans-serif"
    fontSize: "21px"
    fontWeight: 700
    letterSpacing: "-0.9px"
rounded:
  hairline: "2px"
  check: "5px"
  tip: "8px"
  field: "10px"
  tile: "12px"
  float: "14px"
  card: "16px"
  modal: "20px"
  pill: "999px"
spacing:
  xs: "6px"
  sm: "8px"
  tile-gap: "10px"
  stack: "14px"
  card-pad: "18px"
  gutter: "24px"
components:
  button-primary:
    backgroundColor: "{colors.electric-blue}"
    textColor: "{colors.on-accent}"
    typography: "{typography.body-strong}"
    rounded: "{rounded.pill}"
    padding: "0 15px"
    height: "34px"
  button-primary-hover:
    backgroundColor: "{colors.electric-blue-hover}"
  button-secondary:
    backgroundColor: "{colors.sunken}"
    textColor: "{colors.ink}"
    typography: "{typography.body-strong}"
    rounded: "{rounded.pill}"
    padding: "0 15px"
    height: "34px"
  button-secondary-hover:
    backgroundColor: "{colors.hover}"
  button-outline:
    backgroundColor: "{colors.surface}"
    textColor: "{colors.ink}"
    typography: "{typography.body-strong}"
    rounded: "{rounded.pill}"
    padding: "0 15px"
    height: "34px"
  button-outline-hover:
    backgroundColor: "{colors.sunken}"
  button-ghost:
    textColor: "{colors.muted}"
    typography: "{typography.body-strong}"
    rounded: "{rounded.pill}"
    padding: "0 15px"
    height: "34px"
  button-ghost-hover:
    backgroundColor: "{colors.hover}"
    textColor: "{colors.ink}"
  button-danger:
    textColor: "{colors.bad-red-text}"
    typography: "{typography.body-strong}"
    rounded: "{rounded.pill}"
    padding: "0 15px"
    height: "34px"
  button-small:
    rounded: "{rounded.pill}"
    padding: "0 11px"
    height: "28px"
  card:
    backgroundColor: "{colors.surface}"
    rounded: "{rounded.card}"
    padding: "{spacing.card-pad}"
  float-surface:
    backgroundColor: "{colors.surface}"
    rounded: "{rounded.float}"
    padding: "12px 16px"
  modal:
    backgroundColor: "{colors.surface}"
    rounded: "{rounded.modal}"
    padding: "24px"
  text-field:
    backgroundColor: "{colors.sunken}"
    textColor: "{colors.ink}"
    typography: "{typography.body}"
    rounded: "{rounded.field}"
    padding: "8px 12px"
  text-field-focus:
    backgroundColor: "{colors.surface}"
  chip-neutral:
    backgroundColor: "{colors.sunken}"
    textColor: "{colors.muted}"
    typography: "{typography.chip}"
    rounded: "{rounded.pill}"
    padding: "0 9px"
    height: "22px"
  chip-accent:
    backgroundColor: "{colors.electric-blue-soft}"
    textColor: "{colors.electric-blue-text}"
    typography: "{typography.chip}"
    rounded: "{rounded.pill}"
    height: "22px"
  pill-tab:
    textColor: "{colors.muted}"
    typography: "{typography.label}"
    rounded: "{rounded.pill}"
    padding: "0 13px"
    height: "32px"
  pill-tab-selected:
    backgroundColor: "{colors.electric-blue}"
    textColor: "{colors.on-accent}"
  segmented-track:
    backgroundColor: "{colors.sunken}"
    rounded: "{rounded.pill}"
    height: "34px"
  segmented-thumb:
    backgroundColor: "{colors.surface}"
    textColor: "{colors.ink}"
    rounded: "{rounded.pill}"
  toggle-off:
    backgroundColor: "{colors.toggle-off}"
    rounded: "{rounded.pill}"
    width: "38px"
    height: "22px"
  toggle-on:
    backgroundColor: "{colors.electric-blue}"
    rounded: "{rounded.pill}"
    width: "38px"
    height: "22px"
  metric-tile-selected:
    backgroundColor: "{colors.electric-blue-soft}"
    textColor: "{colors.electric-blue-text}"
    rounded: "{rounded.tile}"
    height: "82px"
  nav-capsule:
    backgroundColor: "{colors.sunken}"
    rounded: "{rounded.pill}"
    width: "52px"
  nav-disc:
    backgroundColor: "{colors.electric-blue}"
    textColor: "{colors.on-accent}"
    rounded: "{rounded.pill}"
    size: "40px"
  stat-pill:
    backgroundColor: "{colors.sunken}"
    textColor: "{colors.muted}"
    rounded: "{rounded.pill}"
    padding: "0 12px"
    height: "30px"
---

# Design System: Avix

## Overview

**Creative North Star: "The Quiet Control Room"**

Avix is a performance tool that must cost the game nothing, and its dashboard looks the part: a white canvas (near-black in dark mode), white cards on hairline borders, soft grey sunken tracks, and one electric blue that marks where you are and what to press. Everything else is calm. Big tabular numbers carry the information; colour only speaks when something is boosting or something is wrong. The window is native egui with no web layer, so every token lives in `src/ui/theme.rs` and pages ask for roles (`text()`, `accent()`, `Tone::Good`), never raw colours. The theme switch and the animations setting therefore apply everywhere at once.

The density is comfortable rather than packed: 18 px card padding, 14 px between stacked cards, 13.5 px body type. Controls are full capsules; containers are gently rounded rectangles. A floating vertical icon capsule on the left holds navigation, with a blue disc that glides to the active page. Motion is short (0.12 to 0.32 s, cubic-out), purposeful, fully switchable, and never runs while the window is idle. The world refuses the dark-gamer-neon optimizer and the grey Windows admin panel.

**Key Characteristics:**
- One accent (electric blue) for selection and the single primary action; status colours only for state.
- Flat, bordered cards; shadows only on layers that float above the page.
- Every interactive control is a pill; every container is a 16 px rounded card.
- One grotesk (Geist, four static weights) with tabular digits baked in; Lucide line icons only.
- Light and dark are peers, built from the same role table.
- Motion is optional and idle-silent; live numbers never animate.

## Colors

A near-monochrome cool-neutral palette with a single saturated blue and three reserved status hues. Hex values in the frontmatter are normative; `dark-*` keys are the same roles in the dark palette.

### Primary
- **Electric Blue** (`electric-blue`; dark `dark-electric-blue`): the selection colour and the primary action. Fills the active nav disc, the selected pill tab, the on-state switch, checked checkboxes, slider fill and knob, the text cursor, the chart line and its fill, and the single Primary button on a page. Hovers deepen in light mode and brighten in dark (`electric-blue-hover`).
- **Electric Blue Soft** (`electric-blue-soft`): the tint behind the selected metric tile, Accent chips, the Accent icon badge and the update card. Paired with **Electric Blue Text** (`electric-blue-text`) for readable blue text on a card or tint.
- **Selection** (`selection`): text selection highlight only.

### Status (reserved)
- **Boost Green** (`boost-green` solid, `boost-green-text` for text): a live boost and nothing else. The status dot and tinted boost card while boosting, the green engine dot, success toasts, the green tray tile.
- **Warn Amber** (`warn-amber` / `warn-amber-text`): problems of medium weight. Load values at 70 % and up, ping 40 to 90 ms, limited mode (no admin), warning findings.
- **Bad Red** (`bad-red` / `bad-red-text`): real problems and destructive actions. Load at 90 % and up, ping over 90 ms or timeout, engine offline, error toasts, the Danger button.

Status soft backgrounds are not separate tokens: a tone's soft fill is its solid mixed into the card surface at 10 % (light) or 16 % (dark), and a tinted card's border is that soft fill mixed 22 % toward the solid.

### Neutral
- **Canvas / Surface** (`canvas`, `surface`): pure white in light mode (both), so cards are separated by hairlines, not by tone. In dark mode the canvas (`dark-canvas`) sits one step below the card surface (`dark-surface`).
- **Sunken** (`sunken`): tracks and wells. Nav capsule, segmented track, inactive text fields, Secondary buttons, neutral chips, stat pills, removable chips, the Neutral icon badge.
- **Hover** (`hover`): hover fill for ghost controls, nav items, pill tabs and Secondary buttons.
- **Border / Border Strong** (`border`, `border-strong`): 1 px card and divider hairlines; the stronger step for Outline buttons, floating surfaces, chart baselines and the nav foot rule.
- **Ink** (`ink`): primary text. **Muted** (`muted`): secondary text, labels, inactive icons. **Faint** (`faint`): timestamps, placeholders, the Neutral tone's solid.
- **Toggle Off / Toggle Ring** (`toggle-off`, `toggle-ring`): the off-switch track and its outline.

### Named Rules
**The One Blue Voice Rule.** Electric blue means "selected" or "the one thing to do here". Each page carries at most one Primary button; row and list actions use Secondary, Outline or Ghost.

**The Green Means Boosting Rule.** Green appears only while a boost is live (or to confirm a completed action in a toast). Healthy idle states are neutral or blue, never green.

**The Problems Only Rule.** Amber and red appear only when something needs attention. A load figure stays ink until 70 % (amber) and 90 % (red); nothing is coloured to look busy.

**The Neutral Chip Rule.** Chips are neutral (sunken fill, muted text) unless they report a state that has a tone. Category, count and label chips stay grey.

## Typography

**Display Font:** Geist (with Segoe UI from Windows as fallback, then Lucide for symbols)
**Body Font:** Geist
**Label/Mono Font:** Geist (the Monospace family is also mapped to Geist)

**Character:** One clean grotesk across the whole app, shipped as four static instances (Regular, Medium, SemiBold, Bold) with tabular figures baked in and kerning carried to a legacy kern table so egui renders it correctly. Hierarchy comes from size and weight, with slight negative tracking on larger sizes.

### Hierarchy
- **Display** (SemiBold 600, 26 px, -0.5 px tracking; -0.6 px on the dashboard status headline): page titles and the "Boosting X" / "Watching for games" headline.
- **Metric** (SemiBold 600, 24 px, tabular): big live values in the metric tiles.
- **Headline** (SemiBold 600, 22 px, -0.4 px): dialog and welcome titles; egui's Heading style.
- **Title** (SemiBold 600, 15 px, -0.15 px): card titles.
- **Body** (Regular 400, 13.5 px): all running text, inputs and settings rows. **Body Strong** (Medium 500, 13.5 px): row labels and button text.
- **Label** (Medium 500, 12 px, muted, sentence case): table column heads, field labels, in-card sub-labels. **Small** (Regular 400, 12 px, muted): hints under settings rows and descriptions.
- **Chip** (Medium 500, 11.5 px) and **Caption** (Regular 400, 11 to 11.5 px, muted or faint): chips, timestamps, chart axis labels.
- **Wordmark** (Bold 700, 21 px, -0.9 px, lowercase "avix"): top bar only.

### Named Rules
**The Tabular Figures Rule.** Every number is set in Geist's tabular digits so live values do not jitter as they change; never swap in a proportional-figure font for numbers.

**The Sentence Case Rule.** Labels and headings are sentence case. Small labels sit inside cards or above columns; they never float above a headline as an uppercase tag.

## Layout

A fixed shell around a scrolling content column. The left nav panel is 84 px wide (14 px side margins, 18 px top/bottom) and holds the 34 px logo, then the floating icon capsule (40 px items, 6 px gaps, 6 px padding) with its foot: a hairline, the theme switch, the About heart and the engine status dot. The top bar is 66 px tall: the wordmark, the Ctrl+K search field (42 % of the bar, clamped 180 to 400 px), and right-aligned engine stat pills that drop out below 640 px. Content has a 24 px right gutter and a 1320 px maximum width; settings narrows to 880 px.

The spacing rhythm is 6 / 8 / 10 / 14 / 18 / 24 px: 8 px default item spacing, 10 px between metric tiles, 14 px between stacked cards and below a page header, 18 px card padding and dashboard column gap, 24 px modal padding and content gutter.

Responsive behaviour is decided in code by available content width, not by window presets:
- **Dashboard:** two columns at 1040 px and up (side column 31 % of width, clamped 330 to 400 px, 18 px gap); below that the side column drops under the live card. The status header stacks its action under the title below 760 px.
- **Metric tiles:** four across at 560 px and up, two across below; the chart height drops from 196 px to 150 px at the same point. Health-check tiles follow the same 4-to-2 split.
- **Process table:** drops the PID column below 860 px, GPU below 720 px, Priority below 620 px.
- **Split rows** (label left, control right) stack when the row is narrower than the control plus 220 px.
- **Save bar:** when there are unsaved changes, a 76 px spacer panel reserves the bottom so the floating bar never covers content; toasts lift above it.

## Elevation & Depth

A hybrid: the page itself is flat and uses hairlines and tonal wells for structure; only surfaces that float above the page cast a shadow. Cards never carry a shadow. Shadow colour is black at alpha 24/255 in light mode and 120/255 in dark mode (the `shadow` token, carried in the sidecar).

### Shadow Vocabulary
- **Float** (offset 0 10 px, blur 30 px, `shadow` colour): toasts, the floating save bar, egui windows.
- **Popup** (offset 0 8 px, blur 24 px): menus, combo popups and dropdowns.
- **Modal** (offset 0 16 px, blur 40 px): confirm dialogs and the first-run welcome, over a black backdrop (alpha 90 light, 160 dark).
- **Knob** (1 px drop, black alpha 28 to 30): the white thumbs of switches and sliders only.

### Named Rules
**The Hairline Not Shadow Rule.** Resting containers separate by a 1 px border on the same white; depth is earned only by leaving the page plane.

## Shapes

Two shape families. Controls are full capsules: buttons (radius half their 34 or 28 px height), pill tabs, segmented tracks and thumbs, chips, stat pills, switches, removable chips, the nav capsule and its disc. Containers are rounded rectangles stepped by size: 16 px cards, 20 px modals, 14 px floating surfaces (the save bar is a full 26 px capsule), 12 px metric tiles and menus, 10 px text fields and egui widgets, 8 px chart tooltips, 5 px checkboxes, 2 px slider rails. Icon badges and status dots are circles; the status dot carries a 22 % halo three pixels wider than the dot. The app icon is a squircle tile with a vertical gradient and a white upward chevron over a dot.

**The Capsule And Card Rule.** If you can press it, it is a pill; if it holds things, it is a 16 px card.

## Components

### Buttons
Soft, rounded and quiet, except the one that matters.
- **Shape:** full capsule, 34 px tall (28 px small), 15 px side padding (11 px small), 15 px icon (14 px small) with a 7 px gap; icon-only buttons are circles.
- **Primary:** electric blue fill, white Medium text. One per page.
- **Secondary:** sunken fill, ink text; hover to the hover tone. Used for row actions that need weight (Apply, a fix for a warning finding).
- **Outline:** surface fill with a 1 px strong border; hover to sunken. The default for row and utility actions (Revert, Refresh, Restart engine).
- **Ghost:** transparent, muted text; hover fills and darkens the text. For "See all", "Edit", list reorder and remove.
- **Danger:** red soft fill with red text; destructive confirms and removals.
- **States:** hover fills ease over 0.12 s; pressed mixes 6 % ink into the fill; disabled draws at 45 % opacity; keyboard focus draws a 2 px blue ring (60 % alpha) 2 px outside.

### Chips
- **Style:** 22 px capsule, tone-soft fill, tone-fg text in Chip type, 9 px padding, optional 12 px leading icon.
- **State:** Neutral by default; Accent, Good, Warn and Bad only for a real state. Removable chips are 28 px sunken capsules whose × turns the chip red-soft on hover.

### Cards / Containers
- **Corner Style:** 16 px.
- **Background:** surface; tinted cards use a tone's soft fill (Good for the live boost card, Accent for the update card).
- **Shadow Strategy:** none (see Elevation & Depth).
- **Border:** 1 px `border`; tinted cards use the tone-mixed border.
- **Internal Padding:** 18 px.

### Inputs / Fields
- **Style:** sunken fill, no stroke, 10 px radius, 8 / 12 px padding, Body text, faint placeholder, optional 15 px leading icon.
- **Focus:** fill turns to surface and a 1 px electric-blue stroke appears; the leading icon goes from faint to ink.
- **Combo:** egui combo with a Lucide chevron (down, up when open). **Slider:** 4 px rail (hover tone light, border-strong dark), blue fill, white-rimmed blue knob that grows on hover, value in Medium 13 px to its left, arrow keys step when focused. **Checkbox:** 18 px, 5 px radius, strong-border outline that fills blue with a white check.

### Switches
- 38 x 22 px capsule, white 16 px knob with a soft drop. Off: `toggle-off` track with a 1.5 px `toggle-ring` outline; on: electric blue. The track and knob ease over 0.18 s.

**The Visible Off Rule.** An off switch always carries its ring at 3:1 or better against the card; a pale track alone is not a control.

### Navigation
- **Style:** a floating sunken capsule (52 px wide) of 18 px Lucide icons; the active page is a 40 px electric-blue disc with a white icon that glides between positions (0.32 s). Hover draws a hover-tone disc behind the icon and darkens it to ink. Tooltips name each page; Ctrl+1 to 6 jumps pages.
- **Foot:** a strong hairline, the theme switch (System / Light / Dark cycle with sun-moon, sun and moon icons), a heart that jumps to Settings → About (credits, star, support, contribute), and the engine dot (green boosting, amber limited, blue running, red offline).
- **Segmented control:** sunken capsule track with a sliding thumb (surface with a hairline in light, border-strong in dark), 0.22 s; selected and hovered labels go ink. **Pill tabs:** 32 px filter capsules, outlined at rest, blue-filled and white when selected, optional count.

### Live Metric Card (signature)
A card of four selectable metric tiles (CPU, GPU, Memory, Ping) above an area chart. Each 82 px tile shows a 14 px icon and label, the value in Metric type coloured by load, and a muted sub-line. The selected tile sits on a soft-blue 12 px plate that slides between tiles (0.28 s); hover outlines unselected tiles. The chart draws a 2 px blue line, a hollow end dot at "now", a three-step grid with right-hand axis labels, a "Ns ago" / "now" time axis, a "Collecting samples… n/10" placeholder before ten samples, and a dark rounded hover read-out.

**The Still Numbers Rule.** Live metric values update in place once a second and are never eased; animating them would keep the window repainting and spend CPU while the user games.

**The Seamless Fill Rule.** The chart fill is one mesh whose alpha is linear in height only (strongest at the peak, max 0.26 light / 0.36 dark), with a floor at 22 % of that peak at the baseline, so triangle interpolation leaves no seams and the pale end shows no banding.

### Toasts, Save Bar and Dialogs
Toasts are floating surfaces (340 px content width) stacked bottom-right, with a tone icon (check green, alert red, info blue); they fade in and rise 16 px over 0.3 s. The save bar is a floating capsule holding "Save & apply" as that view's primary action. Dialogs use the modal frame; a destructive confirm swaps the Primary for Danger.

### Motion
All motion is cubic-out: hover 0.12 s, egui animation time and checkbox 0.14 s, switch and pill-tab 0.18 s, segmented thumb 0.22 s, metric plate 0.28 s, page entrance and toasts 0.3 s (fade plus a 10 px rise), nav disc 0.32 s.

**The Optional Motion Rule.** With Settings → Animations off, every helper snaps to its end state and animation time is zero; with it on, nothing repaints continuously while idle.

## Do's and Don'ts

### Do:
- **Do** ask the theme for roles (`text()`, `muted()`, `accent()`, `Tone::*`) and never write a hex value in a page; light and dark must stay one switch.
- **Do** keep one Primary button per page and make row actions Secondary, Outline or Ghost.
- **Do** reserve green for a live boost and amber/red for problems (load 70 % / 90 %); keep everything else neutral or blue.
- **Do** draw an off switch with its ring at 3:1 or better against the card.
- **Do** update live metric values in place without easing, and route every animation through `ease_bool`, `ease_value` or `entrance` so the Animations setting can turn it off.
- **Do** build containers as 16 px cards with an 18 px pad and a 1 px border, and controls as full capsules.
- **Do** decide responsive changes by available content width (1040 / 760 / 560 px on the dashboard, 860 / 720 / 620 px in the process table, 880 px settings maximum).
- **Do** use Lucide line icons from the embedded subset and Geist's tabular figures for every number.

### Don't:
- **Don't** add a second accent colour or use blue as decoration; it marks selection and the primary action only.
- **Don't** colour healthy or idle states green, or tint chips that have no state.
- **Don't** put a shadow on a resting card; shadows belong to toasts, the save bar, popups and dialogs.
- **Don't** ease or tween live readings, or add any animation that repaints while the window is idle.
- **Don't** place uppercase labels or tags above headings; labels stay sentence case inside cards and table heads.
- **Don't** use emoji, Unicode symbols or a second icon set in place of Lucide icons.
- **Don't** drift toward the dark-gamer-neon optimizer look (glows, neon, black-and-red) or the grey Windows admin panel.
