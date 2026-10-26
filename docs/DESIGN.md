# Tandem — design system

Source of truth: the design canvas, "Tandem — App Design" in Claude. Its files are in `design/`: `Main.dc.html` holds every screen, switched by its `screen` prop. Built with the taste-skill app rules (style "framed-sheet") and Apple's Human Interface Guidelines.

## Shell

- **Tinted L-frame:** the sidebar (228px) and the top bar (48px) share the `--ground` colour, with no rule between them.
- **Content sheet:** `--surface`, inset 8px from the right and bottom edges, radius 16, 1px `--border`, **no shadow**, padding 24.
- **Shadows** appear only on things that float: menus, dialogs, the tray flyout and notifications.
- **Search** is centred on the **window** (440×32, pill), not on the sheet.
- **One left edge per page:** the title, cards and table text all align to the sheet padding.

## Tokens

| Token | Light | Dark |
|---|---|---|
| ground | #f6f7f8 | #161719 |
| surface | #ffffff | #1d1f21 |
| raised | #f3f4f6 | #232528 |
| active | #e9ebed | #2c2f32 |
| border | #e6e8ea | #2d3033 |
| rule | #eff0f2 | #26282b |
| control | #cbcdd1 | #4a4d52 |
| text | #131416 | #ececee |
| muted | #5b5e62 | #a3a6ab |
| faint | #66696e | #94989d |
| primary-bg / ink | #131416 / #fff | #ececee / #131416 |
| claude | #b4532c | #e38c64 |
| codex | #2160c4 | #76a5f2 |
| positive | #1e7b4f | #5cc28b |
| negative | #c4242a | #f2726f |
| focus | #0a5dc2 | #8ab4ff |
| switch off / on / knob | #7d8086 / #1e7b4f / #fff | #6b6f75 / #ececee / #1d1f21 |
| bar track / fill | #eceef0 / #3d4045 | #2e3134 / #c9ccd0 |

**Profile tints** (background / ink), light and dark:

| Tint | Light | Dark |
|---|---|---|
| graphite | #e9ebed / #353736 | #33363a / #dcdee1 |
| blue | #e2ecfb / #1d4c8c | #1f3350 / #b7d0f6 |
| green | #dff2e7 / #1d6840 | #1c3a2a / #a9dfbf |
| amber | #f9eed6 / #744a05 | #3f3218 / #efd29a |
| rose | #fae3e8 / #8c2241 | #45212b / #f4bccb |
| teal | #dcf1ef / #145f5a | #173a38 / #a3dcd6 |

## Type

- **Faces:**
  - Instrument Sans (400, 500, 600) for everything.
  - IBM Plex Mono only for paths, env vars and shortcuts.
- **Sizes:** title 22/600 (−0.015em) · section 16/600 · body 14 · small 12–13. Usage and other numbers use `tabular-nums`.
- **Case:** sentence case everywhere. No uppercase labels, no eyebrows.

## Components

- **Buttons:** 32px pill. There are four kinds:
  - Default: surface with a `control` border.
  - Primary: `primary-bg`; at most one per screen.
  - Quiet: transparent.
  - Danger: `negative`, with `#fff` text in light mode and `#131416` in dark.
  
  A disabled primary uses `raised` / `muted` / `control`, never opacity.
- **Nav item:** 32px, radius 8. The active item is an `active` pill with weight 500.
- **Tabs:** underline, 2px in ink.
- **Segmented control:** a `raised` track with a `surface` selected pill.
- **Profile table row:** 60px tall. Columns: 28px · 1fr · 64px · 92px · 164px · 164px · 104px · 32px, with a 12px gap.
- **Avatar:** 32px circle with a tint and initial (24px in the tray and chips).
- **Usage bar:** 6px track, the fill is the true ratio. At 85% or more the bar and number turn `negative` and add "near limit".
- **Status:** an 8px dot plus a word (Running, Idle, Signed out). Status is never shown by colour alone.
- **Sheets:** 480px wide, radius 16, a floating shadow, the backdrop at `overlay`.
- **Menus:** 236px, radius 12, 32px items.
- **Toast:** a pill at the bottom of the sheet, centred, auto-hides after 2.6s.
- **Focus:** a 2px `focus` outline, offset 2.

## Logo — "C · Side by side"

Two offset vertical pills in a 48×48 box:
```svg
<rect x="7" y="4" width="15" height="32" rx="7.5" fill="#b4532c"/>
<rect x="26" y="12" width="15" height="32" rx="7.5" fill="#2160c4"/>
```
- **Dark:** #e38c64 and #76a5f2.
- **Tinted:** #ececee, with the second pill at 55%.
- **Small sizes (24 and 16px):** crop the viewBox to `5 2 38 44`.
- **Tray glyph:** two pill outlines with a 1.5px stroke.
- **Wordmark:** "Tandem" in Instrument Sans 600, −0.025em.

## Motion

- Feedback only: hover, press (scale .98), and menus or sheets rising 8px over 180ms.
- Nothing performs an entrance animation.
- Respect `prefers-reduced-motion`.
