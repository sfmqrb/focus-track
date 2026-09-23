# Usage

## Commands

| command | shows |
|---|---|
| `focus-track` | today: time per app, goal, trend vs. your usual |
| `focus-track dashboard` | everything, live, full screen |
| `focus-track week` · `heatmap` | the last days, stacked by app · hour by hour |
| `focus-track timeline` · `streaks` · `graph` | minute by minute · longest stretches · app switching |
| `focus-track pages` | websites with time and URL (`-s text` to filter) |
| `focus-track apps` | every window class seen, its name and category |
| `focus-track doctor` | is recording working? |
| `focus-track export` | CSV of everything (`--json` works on most views) |

Useful flags: `-d 2026-09-20` another day, `-D 30` more days, `-c` group by category, `-g 5` daily goal in hours.
`focus-track --help` lists everything; `focus-track completions <shell>` prints shell completions.

## Dashboard keys

Every list works the same way.

| key | does |
|---|---|
| `↑ ↓` `PgUp PgDn` `Home End` | move the selection |
| `⏎` or click | open: an app's details, or a page in your browser |
| `esc` | back one level (quits from the overview) |
| `tab` · `1`–`7` · `p` | next panel · zoom a panel · pages |
| `← →` · `t` | previous / next day · today |
| `c` · `g` | apps ↔ categories · graph mode |
| `?` · `q` | help · quit |

## Reading the charts

Each symbol means one thing everywhere:

| symbol | means |
|---|---|
| `■■■■■` | how much time (longer = more) |
| `●●●··` | when you were there: one dot per minute (timeline) or hour (heatmap); `·` = nothing |
| `⣀⣤⣶⣿` | a trend through the day |
| highlighted row | the selection |
| `▲ ▼ –` | more / less / about your usual by this time of day |

## What counts

- **Focused**: a window had focus and you weren't idle or locked.
- **Watching**: the focused app was playing audio or video (detected with `pactl`), even if you didn't touch anything.
- **Away**: idle, locked or suspended; not counted.
