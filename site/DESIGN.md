# Shuusshy website: design

The website looks like the app: whoever downloads Shuusshy should recognise it at once. Tokens
come from the app's design system (`src/index.css`).

## Principles
- **Minimal:** one idea per section, generous space, large type, few boxes. Lists and hairlines
  instead of cards where a card adds nothing.
- **Dark liquid glass:** translucent layered panels with a gradient edge (bright top left) over
  a soft, still colour field. One accent colour (cyan); violet only for Pro; amber only for
  noise and beta.
- **Interactive with meaning:** the hero lets visitors flip noise cancellation on a live
  waveform; screenshots tilt toward the pointer; buttons lean toward it. Every motion answers an
  input or explains the product.
- **Real things only:** screenshots of the real interface, figures from the QA report labelled as
  measured, no testimonials, logos or invented numbers. The waveform is labelled an illustration.
- **Private like the app:** no trackers, cookies, web fonts or third-party scripts. One local
  script; the page is complete without it.

## Colour (dark only, like the app)
| Token | Value | Role |
|---|---|---|
| `--base` | `#06080c` | Page background |
| `--ink` | `#f3f6fa` | Headings, primary text (≥ 15:1) |
| `--ink-2` | `#c7d0db` | Body text (≥ 11:1) |
| `--muted` | `#8f9cab` | Secondary text (≥ 6.5:1) |
| `--line` | `rgb(255 255 255 / .08)` | Hairlines |
| `--accent` | `#39d5ff` | Links, primary buttons, voice |
| `--accent-2` | `#a594ff` | Pro |
| `--warn` | `#ffbd5c` | Noise, beta |

## Type
System fonts: Segoe UI Variable / San Francisco / system-ui. Hero 3–5.6 rem at weight 650 with
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
- Backdrop blur only on the sticky header. The colour field is one still layer: three drifting
  layers cost frames (measured).
- No `content-visibility`: it made jump links land in the wrong place.
- Everything stops for `prefers-reduced-motion`; the waveform then shows a still frame.

### Measured (headless Edge, full-page scroll in 5 s)
- Frame rate uncapped: 212–237 fps; 95 % of frames ≤ 7.2 ms; about 1 % over the 8.3 ms budget
  of a 120 Hz display. Hero waveform alone: 367–390 fps (about 2.6 ms per frame).
- At 60 Hz: 59.6 fps; 58 fps with the CPU slowed 4×; no long frames.

## Components
Glass panel · primary, ghost and Pro pill buttons · chip · liquid switch · platform switch with
sliding pill · disclosure (`<details>`) · screenshot frame with tilt · stepper · stat row.
