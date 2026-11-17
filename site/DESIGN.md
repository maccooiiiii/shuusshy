# Shuusshy website: design

The website looks like the app: whoever downloads Shuusshy should recognise it at once. Tokens
come from the app's design system (`src/index.css`).

## Principles
- Dark "liquid glass": translucent panels with a thin light rim over a near-black base and a
  soft accent light. One accent colour (cyan); violet only for Pro.
- Real things only: screenshots of the real interface, figures from the QA report labelled as
  measured, no testimonials, logos or invented numbers.
- Private like the app: no trackers, cookies, web fonts or third-party scripts. One small local
  script (OS detection, manual platform switch); the page works without it.

## Colour (dark only, like the app)
| Token | Value | Role |
|---|---|---|
| `--base` | `#07090d` | Page background |
| `--panel` | `#10151c` | Panels (with transparency) |
| `--line` | `rgb(255 255 255 / .1)` | Borders, rims |
| `--ink` | `#f2f6fb` | Text (≥ 15:1 on base) |
| `--muted` | `#9aa7b6` | Secondary text (≥ 7:1 on base) |
| `--accent` | `#39d5ff` | Links, primary buttons, highlights |
| `--accent-2` | `#a594ff` | Pro |
| `--warn` | `#ffbd5c` | Beta / caution labels |

## Type
System fonts: Segoe UI Variable / San Francisco / system-ui. Display headings 600 weight with
tight tracking; small uppercase "tech" labels with wide tracking (as in the app). Body 16–17 px,
line height 1.6.

## Shape and depth
Radius 14 px for panels, 999 px for buttons and chips. Depth comes from a light top rim and a
pointer-following glow, not shadows: large soft shadows and backdrop blur on every panel cost
dropped frames while scrolling (measured), so only the sticky header blurs.

## Motion
Fluid but cheap: only `transform`, `translate` and `opacity` are animated, so motion keeps up
with 60/120/144 Hz displays.
- Sections rise into view with CSS scroll-driven animations (run by the compositor, tied to
  the scroll position). Unsupported browsers show the content directly.
- The hero enters once on load; the banner screenshot floats and tilts toward the pointer.
- Panels lift and light up under the pointer; buttons press with a spring.
- The manual's platform switch slides; manual topics and FAQ answers open with a height
  animation; the nav underlines the section you are reading; a progress line runs under the
  header.
- Pointer and scroll handlers only record values; one `requestAnimationFrame` applies them.
- No `content-visibility`: it made jump links land in the wrong place.
- Everything stops for `prefers-reduced-motion`.

Measured with headless Edge, scrolling the whole page in 5 s: 59.8 fps with no long frames, and
58 fps with the CPU slowed 4× (the first version: 55 fps, frames up to 133 ms).

## Components
Glass panel · primary (accent) and secondary (glass) pill buttons · chip · platform switch ·
disclosure (`<details>`) · screenshot frame.
