# Security and privacy

## What focus-track stores, and where

Everything stays on your machine. focus-track has no network code at all.

| What | Where | Why |
|---|---|---|
| focused window class, title, times, whether media was playing | `~/.local/state/focus-track.db` (mode `0600`) | the data every view is drawn from |
| URL of a browser page | same database | only once the exact page title is found in your browser's own history |
| temporary copies of browser history databases | `~/.cache/focus-track/` (dir `0700`, files `0600`) | browsers keep their history locked, so it is copied to be read |
| your config | `~/.config/focus-track/config.toml` | names and categories, written by you |

**Private / incognito windows are never stored.** Their pages are not in browser history, so their titles
are only held in memory and dropped after five minutes; the time still counts, with no title or URL.

Delete everything: `rm -r ~/.local/state/focus-track.db* ~/.cache/focus-track`.

## What it runs

Only these helpers, found on your `PATH`, each without a shell and with fixed arguments:
`hyprctl` (active window), `omarchy-shell` or `loginctl` (idle/lock state), `pactl` (is media playing),
`notify-send` (the end-of-day summary), and `xdg-open` (only when you press ⏎ on an `http(s)` link in the dashboard).

Window titles, URLs and theme colors are stripped of control characters before they reach your terminal,
so a web page cannot inject escape sequences into the dashboard.

## Reporting a vulnerability

Please use GitHub's private vulnerability reporting (the **Security** tab of this repository → *Report a vulnerability*)
rather than a public issue.
