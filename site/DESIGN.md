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
Radius 14 px for panels, 999 px for buttons and chips. Depth comes from rims and one soft
shadow, not stacked shadows.

## Motion
Short fades and lifts on hover; the emblem animates like the app's. Everything stops for
`prefers-reduced-motion`.

## Components
Glass panel · primary (accent) and secondary (glass) pill buttons · chip · platform switch ·
disclosure (`<details>`) · screenshot frame.
