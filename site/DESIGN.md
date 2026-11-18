# Shuusshy website: design

The website looks like the app: whoever downloads Shuusshy should recognise it at once. Tokens
come from the app's design system (`src/index.css`).

## Principles
- **Two-tone grey gradients that move as you scroll:** light and dark sections take turns. Each
  section is a window (clip-path) onto its own fixed gradient: two pale greys behind light
  sections, two dark greys behind dark ones, picked at random for every visit within ranges that
  keep text at 4.5:1. Two soft glows, one per grey, circle on their own paths as the page
  scrolls (springs, so they glide at the display's rate) and lean toward the pointer. Fine grain
  is baked into the still background. Because every section shows only its own gradient, text
  always sits on its own tone.
- **Liquid glass:** panels are clear glass with a bright rim. Buttons are drops of glass: a rim
  lit from the top left, an inner highlight, a refraction glow on the bottom edge and a sheen that
  follows the pointer; pressing swells and brightens them. Selection markers (the Windows/macOS
  switch, feature tabs, FAQ filters) are glass lenses that slide on springs, stretch along their
  path while moving and settle with a small wobble.
- **Cyan is the voice:** the main action is cyan-tinted glass; cyan also marks signal and
  selection. Amber marks noise.
- **Interactive with meaning:** the hero's switch warms the room while noise cancellation is
  off; the signal path explains each step; features are tabs on one stage; the user guide has a
  sticky topic rail; the FAQ is a magazine (topic filters, contents in a book face, one open page).
- **Both downloads in the hero:** the visitor's own system first, the other beside it.
- **Real things only:** screenshots of the real interface, measured figures labelled as such.
- **Private like the app:** no trackers, cookies, web fonts or third-party scripts.

## Colour
| Token | Light sections | Dark sections | Role |
|---|---|---|---|
| `--ink` | `#121417` | `#f2f3f5` | Headings, primary text |
| `--ink-2` | `#2a2d32` | `#cfd2d7` | Body text |
| `--muted` | `#4a4e55` | `#a9adb4` | Secondary text (≥ 4.5:1 on every shade of its tone) |
| `--accent` | `#075a75` | `#5fd8ff` | Voice: links, labels, selection |
| `--warn` | `#ffbd5c` | `#ffbd5c` | Noise, beta |

Pale greys 208–246, dark greys 14–50 (per channel). The main action is cyan-tinted glass with dark ink on both tones.

## Type
System fonts: Segoe UI Variable / San Francisco / system-ui. FAQ magazine questions and page in a book face (Iowan Old Style / Palatino Linotype /
Georgia), everything else in the system sans. Hero 3–5.6 rem at weight 650 with
-0.045em tracking; section titles 2–3.2 rem; body 17 px, line height 1.6; small uppercase labels
with wide tracking (as in the app).

## Shape and depth
Radius 20 px for panels, 14 px for inner surfaces, 999 px for buttons and chips. Depth comes from
the gradient edge, an inner top highlight and a pointer-following glow, not stacked shadows.

## Motion: built for 120 Hz
- Only `transform`, `translate`, `scale` and `opacity` animate.
- Pointer-driven motion (tilt, magnetic buttons, glow) runs on time-based springs in one
  `requestAnimationFrame` loop at the display's own rate, never on CSS transitions, so it never
  trails the pointer. The loop sleeps when nothing moves; the waveform pauses off screen.
- Sections rise into view with CSS scroll-driven animations (compositor). The hero enters once.
- Small UI motion (switch knob, platform pill, chevrons, nav underline) uses springy CSS
  transitions; topics and FAQ answers open with an interruptible height animation.
- Backdrop blur only on the sticky header. The colour field is one oversized gradient layer
  moved by transform, plus a warm layer that is hidden whenever its opacity is 0. Measured:
  four separate pool layers cost about 30 % of the frame rate, a grain texture about 75 %,
  and 3D-stacked screenshots (preserve-3d) a further share; all three were removed.
- Inactive showcase screenshots are `visibility: hidden` after they fade, so they leave the
  compositor. The signal pulse is a CSS animation that pauses off screen.
- No `content-visibility`: it made jump links land in the wrong place.
- Everything stops for `prefers-reduced-motion`; the waveform then shows a still frame.

### Measured (headless Edge, full-page scroll in 5 s; before the interactive layout)
- Frame rate uncapped: 212–237 fps; 95 % of frames ≤ 7.2 ms; about 1 % over the 8.3 ms budget
  of a 120 Hz display. Hero waveform alone: 367–390 fps (about 2.6 ms per frame).
- At 60 Hz: 59.6 fps; 58 fps with the CPU slowed 4×; no long frames.

## Components
Glass panel · signal path with pulse and detail panel · feature tabs with sliding glass drop
and stage · primary, ghost and Pro pill buttons · chip · liquid switch · platform switch with
sliding pill · disclosure (`<details>`) · screenshot frame with tilt · stepper · stat row.

### Interactive layout (November 2026, same method, same machine and session)
- New page 120–150 fps uncapped against 130–165 fps for the previous page (software
  rendering); hero idle 320–330 fps. In the browser at 60 Hz: 60 fps, no frame over 17.1 ms.

### Two-tone gradients (November 2026, headless Edge, same session)
- 145–154 fps uncapped against 178–184 fps for the previous page; 95 % of frames ≤ 10 ms.
  Measured costs: the moving glows about 15 %, a separate grain layer about 8 % (now baked into
  the still background), oversized backgrounds a further share (now screen-sized).
