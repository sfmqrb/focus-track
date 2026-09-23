# focus-track

See where your time goes on [Hyprland](https://hyprland.org). focus-track records which window has focus,
skips the time you're away, and shows it all in a btop-style terminal dashboard.

Everything stays on your machine: no account, no cloud, no network access at all.

```
 focus-track ‹ tuesday · Tue 22 Sep ›   ● tracking       9h11 of 6h00 goal   by app             18:05
╭─┤¹by app├────────────────────────────────────────────────────────────────────────────────────────────╮
│ 9h11 focused · 70 switches · since 11:08                                                             │
│ goal ■■■■■■■■■■■■■■■■■■■■■■■■■■■■■■■■■■■■■■■■■■■■■■■■■■■■■■■■■■■■■■■■■■■■■■■■■■■■■■■■■ of 6h00  ✓    │
│                                                                                                      │
│ Terminal ■■■■■■■■■■■■■■■■■■■■■■■■■■■■■■■■■■■■■■■■■■■■■■■■■■■■■■■■■■■■■■■■■■■■■■■■■■■■■  2h54  32% –  │
│ firefox  ■■■■■■■■■■■■■■■■■■■■■■■■■■■■■■■■■■■■■■■■■■■■■■■■■■■■■■■■■■■■■■■■■■■■■■■■■■■■■  1h40  18% –  │
│ code     ■■■■■■■■■■■■■■■■■■■■■■■■■■■■■■■■■■■■■■■■■■■■■■■■■■■■■■■■■■■■■■■■■■■■■■■■■■■■■  1h35  17% –  │
│ mpv      ■■■■■■■■■■■■■■■■■■■■■■■■■■■■■■■■■■■■■■■■■■■■■■■■■■■■■■■■■■■■■■■■■■■■■■■■■■■■■   55m  10% –  │
│ Telegram ■■■■■■■■■■■■■■■■■■■■■■■■■■■■■■■■■■■■■■■■■■■■■■■■■■■■■■■■■■■■■■■■■■■■■■■■■■■■■   41m   8% –  │
│ obsidian ■■■■■■■■■■■■■■■■■■■■■■■■■■■■■■■■■■■■■■■■■■■■■■■■■■■■■■■■■■■■■■■■■■■■■■■■■■■■■   36m   7% –  │
│ slack    ■■■■■■■■■■■■■■■■■■■■■■■■■■■■■■■■■■■■■■■■■■■■■■■■■■■■■■■■■■■■■■■■■■■■■■■■■■■■■   18m   3% –  │
│ zoom     ■■■■■■■■■■■■■■■■■■■■■■■■■■■■■■■■■■■■■■■■■■■■■■■■■■■■■■■■■■■■■■■■■■■■■■■■■■■■■   17m   3% –  │
│ spotify  ■■■■■■■■■■■■■■■■■■■■■■■■■■■■■■■■■■■■■■■■■■■■■■■■■■■■■■■■■■■■■■■■■■■■■■■■■■■■■   10m   2% ▼  │
╰──────────────────────────────────────────────────────────────────────────────────────────────────────╯
╭─┤³activity · switches├───────────────────────────────────────────────────────────────────────────────╮
│ app switches per hour · peak 16/h at 11:49 · tall = scattered, low = deep focus                      │
│ ⠀⠀⠀⠀⠀⠀⠀⠀⡇⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⡇⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⢸⡇⢸⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⣿⢸⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⢸⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀ │
│ ⠀⠀⠀⠀⠀⠀⠀⢰⣷⡆⠀⠀⠀⠀⠀⠀⠀⠀⣶⡆⠀⠀⠀⠀⠀⠀⠀⠀⠀⣶⣷⠀⠀⠀⠀⠀⢰⡆⡆⠀⠀⠀⢰⣾⣷⣾⣶⠀⠀⣶⡆⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⣿⣾⡆⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⢰⣾⣶⣶⠀⠀⠀⠀⠀⠀⠀⠀⡆⠀⠀⠀⠀ │
│ ⠀⠀⠀⠀⠀⠀⠀⣼⣿⣧⠀⠀⠀⢠⡄⣤⠀⣤⣿⣧⠀⠀⣤⣤⣤⡄⠀⠀⢠⣿⣿⡄⡄⣤⡄⡄⣼⣧⣧⢠⡄⠀⣼⣿⣿⣿⣿⣤⣤⣿⣧⡄⣤⣤⡄⠀⣤⡄⡄⢠⣤⣤⣤⢠⡄⢠⢠⣤⢠⢠⣿⣿⡇⣤⠀⠀⠀⠀⠀⢠⣤⠀⢠⣼⣿⣿⣿⠀⡄⣤⣤⣤⣤⣤⢠⣧⣤⠀⠀⠀ │
│ ⠀⠀⠀⢀⣀⣀⣀⣿⣿⣿⡀⠀⣀⣸⣇⣿⣀⣿⣿⣿⣀⣀⣿⣿⣿⣇⣀⠀⣸⣿⣿⣇⣇⣿⣇⣇⣿⣿⣿⣸⣇⣀⣿⣿⣿⣿⣿⣿⣿⣿⣿⣇⣿⣿⣇⣀⣿⣇⣇⣸⣿⣿⣿⣸⣇⣸⣸⣿⣸⣸⣿⣿⣇⣿⣀⡀⠀⠀⢀⣸⣿⣀⣸⣿⣿⣿⣿⣀⣇⣿⣿⣿⣿⣿⣸⣿⣿⡀⣀⣀ │
│ ⠀⠀⠀⢸⣿⣿⣿⣿⣿⣿⡇⠀⣿⣿⣿⣿⣿⣿⣿⣿⣿⣿⣿⣿⣿⣿⣿⠀⣿⣿⣿⣿⣿⣿⣿⣿⣿⣿⣿⣿⣿⣿⣿⣿⣿⣿⣿⣿⣿⣿⣿⣿⣿⣿⣿⣿⣿⣿⣿⣿⣿⣿⣿⣿⣿⣿⣿⣿⣿⣿⣿⣿⣿⣿⣿⡇⠀⠀⢸⣿⣿⣿⣿⣿⣿⣿⣿⣿⣿⣿⣿⣿⣿⣿⣿⣿⣿⡇⣿⣿ │
│ 11       12        13        14       15        16        17        18       19        20        21  │
╰──────────────────────────────────────────────────────────────────────────────────────────────────────╯
╭─┤⁴timeline├──────────────────────────────────────────────────────────────────────────────────────────╮
│    :00            :15            :30            :45                                                  │
│ 18 ····●●●●●●●●●●●●●●●●●●●●●●●●●●●●●●●●●●●●●●●●●●●●●●●●●●●●·●●●                                      │
│ 19 ●●●●●●●●●●●●●●●●●●●●●●●●●●●●●●●●●●●●●●●●●●●●●●●●●●●●●●●●●●●●                                      │
│ 20 ●·●●●●●●●●●●●●●●●●●●●●●●●●●●●●●●●●●●●●●●●●●●●●●●●●●●●●●●●●●●                                      │
│ 21 ●●●●●●●●●●●●●●●●············································                                      │
│                                                                                                      │
│ ● Terminal  ● firefox  ● code  ● mpv  ● Telegram  ● obsidian  ● slack  ● zoom                        │
╰──────────────────────────────────────────────────────────────────────────────────────────────────────╯
 ↑↓ select  ⏎ details  tab panel  1-7 zoom  ←→ day  c categories  g graph  ? help  q quit
```

<sub>Demo data. Colors follow your terminal or Omarchy theme.</sub>

## Features

- **Live dashboard** (`focus-track dashboard`): today, streaks, an activity graph, a minute-by-minute
  timeline, the week and an hour-by-hour heatmap. Zoom any panel, open any app to see its windows,
  go back and forth between days. Keyboard and mouse.
- **Honest numbers**: idle, locked and suspended time isn't counted. Time with video or a call playing in the
  focused window counts, and is shown separately as *watching*.
- **Pages**: time per website, with the URL (from your browser's own history), for Chromium, Brave, Chrome and
  Firefox. **Private windows are never recorded.**
- **Your names and categories**: rename or merge apps, and group time into categories like work, chat
  and media, using rules you write (never guesses, never AI).
- **Health check**: `focus-track doctor` tells you if the recorder is running, whether anything was missed, and
  what's uncategorized.
- **Scriptable**: `--json`, CSV `export`, a Waybar-style `bar` module, and an Omarchy bar widget.

## Install

**Arch Linux (AUR)**

```sh
yay -S focus-track        # or paru, or: git clone https://aur.archlinux.org/focus-track.git && cd focus-track && makepkg -si
```

**Any Linux, prebuilt binary** (x86_64 and aarch64, static; installs to `~/.local/bin`, no sudo)

```sh
curl -fsSL https://raw.githubusercontent.com/sfmqrb/focus-track/main/install.sh -o install.sh
less install.sh    # read it first
sh install.sh
```

The script checks each download against the release's `SHA256SUMS`. The binaries are built by GitHub Actions
from a tagged commit and carry a build attestation:
`gh attestation verify ~/.local/bin/focus-track --repo sfmqrb/focus-track`.

**From source** (Rust 1.85+)

```sh
cargo install --locked --git https://github.com/sfmqrb/focus-track
```

## Start recording

The recorder is `focus-track daemon`. Run it with your session, either as a systemd user service:

```sh
systemctl --user enable --now focus-track      # the AUR package and install.sh provide the unit
```

or from Hyprland's config:

```
exec-once = focus-track daemon
```

Then run `focus-track doctor` to check that everything works.

## Use

```sh
focus-track                 # today, as a bar chart
focus-track dashboard       # the live dashboard (press ? for help)
focus-track week            # the last 7 days
focus-track pages -s docs   # pages you visited, filtered
focus-track -c              # any view, grouped by category
focus-track apps            # every app seen: its class, name and category
focus-track --help          # everything else
```

In the dashboard every list works the same way:

| key | does |
|---|---|
| `↑ ↓` `PgUp PgDn` `Home End` | move the selection |
| `⏎` / click | open: an app's details, or a page in your browser |
| `esc` | back one level |
| `tab`, `1`–`7`, `p` | switch panel, zoom a panel, pages |
| `← →`, `t` | previous / next day, today |
| `c`, `g` | apps ↔ categories, graph mode |
| `?`, `q` | help, quit |

**How to read it.** Each symbol means one thing everywhere:
`■■■` meters show *how much* time; `●` dots show *when* you were there (one per minute in the timeline, one per hour
in the heatmap), with `·` for nothing; braille graphs `⣀⣤⣶` show a trend over the day; the reversed row is the selection;
`▲ ▼ –` compare with your usual by this time of day.

## Configure

`focus-track config` creates `~/.config/focus-track/config.toml` and prints its path. Changes apply to all
history at once, since nothing about them is stored.

```toml
[names]                          # rename window classes; globs work; same name = merged
"com.mitchellh.ghostty" = "Terminal"
"chromium" = "Browser"
"brave-browser" = "Browser"

[categories.work]
apps  = ["Terminal", "code"]     # names or classes, globs work
sites = ["github.com"]           # page URLs; win over apps

[categories.media]
apps  = ["mpv"]
sites = ["youtube.com"]          # so YouTube in the browser is media, not work
```

Everything else is `other`. `focus-track apps` and `focus-track doctor` show what isn't categorized yet.

## Omarchy

- **Bar widget**: a small ring of today's apps (or categories), with a dot that breathes while you're tracked.
  Copy [`extras/omarchy/focus.qml`](extras/omarchy/focus.qml) to `~/.config/omarchy/bar/modules/`, add
  `{ "id": "focus", "type": "qml" }` to the bar layout in `~/.config/omarchy/shell.json`, and restart the shell.
  Click it to open the dashboard.
- **Colors** follow your current Omarchy theme; elsewhere the terminal's own colors are used.
- **Idle and lock** come from the Omarchy shell; on other setups from logind (hypridle, swayidle and most
  lockers report there).

## Privacy and security

See [SECURITY.md](SECURITY.md) for exactly what is stored, where, what the program runs, and how to delete it
all. In short: one private SQLite file in `~/.local/state`, nothing sent anywhere, and no titles from private
windows.

## Uninstall

```sh
systemctl --user disable --now focus-track
rm ~/.local/bin/focus-track ~/.config/systemd/user/focus-track.service   # or: pacman -R focus-track
rm -r ~/.local/state/focus-track.db* ~/.cache/focus-track ~/.config/focus-track   # your data and config
```

## License

MIT
