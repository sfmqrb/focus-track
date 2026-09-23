# Install

## Arch Linux

Build and install the package from this repository (it's not on the AUR yet):

```sh
git clone https://github.com/sfmqrb/focus-track.git
cd focus-track/packaging/aur
makepkg -si
```

Installs `/usr/bin/focus-track`, a systemd user unit, shell completions (bash, zsh, fish) and the Omarchy bar widget
in `/usr/share/focus-track/omarchy/`.

## What it needs

- **Hyprland** (the recorder listens to its events). Other compositors aren't supported.
- Nothing else is required. These make it better when present: `pactl` (PipeWire or PulseAudio; counts playing
  video and calls as *watching*), `notify-send` (the evening summary), `xdg-open` (open a page from the dashboard),
  and `omarchy-shell`, or logind idle hints from hypridle/swayidle, to skip time you're away.
- A terminal with UTF-8. Colors follow your Omarchy theme, otherwise your terminal's palette (`NO_COLOR` turns them off).

## Prebuilt binary (any Linux, x86_64 or aarch64)

```sh
curl -fsSL https://raw.githubusercontent.com/sfmqrb/focus-track/main/install.sh -o install.sh
less install.sh        # it's short; read it first
sh install.sh
```

It installs to `~/.local/bin` (no sudo, no Rust), checks the download against the release's `SHA256SUMS`, and adds
bash/fish completions. It uses `curl` or `wget`, whichever you have, and follows `XDG_DATA_HOME`/`XDG_CONFIG_HOME`.
Where systemd runs it also writes a user unit (not enabled); elsewhere it tells you the `exec-once` line to use.
Options: `FOCUS_TRACK_VERSION=v0.2.0`, `FOCUS_TRACK_BIN_DIR=~/bin`. zsh: `focus-track completions zsh > _focus-track`
into a directory on your `$fpath`.

The binaries are built by GitHub Actions from a tagged commit. To check a binary came from there:

```sh
gh attestation verify ~/.local/bin/focus-track --repo sfmqrb/focus-track
```

## From source

Rust 1.85 or newer:

```sh
cargo install --locked --git https://github.com/sfmqrb/focus-track
```

## Start recording

The recorder is `focus-track daemon`; run it with your session. It finds Hyprland by itself, even when started
without `HYPRLAND_INSTANCE_SIGNATURE` or `XDG_RUNTIME_DIR`, and waits (saying so) until Hyprland is up. With systemd:

```sh
systemctl --user enable --now focus-track
```

Or from Hyprland's config:

```
exec-once = focus-track daemon
```

`focus-track doctor` confirms it's running and shows anything that needs attention.

## Omarchy bar widget

A small ring of today's apps (or categories) with a dot that breathes while you're tracked; click it for the dashboard.

1. Copy [`extras/omarchy/focus.qml`](../extras/omarchy/focus.qml) (Arch package: `/usr/share/focus-track/omarchy/focus.qml`)
   to `~/.config/omarchy/bar/modules/focus.qml`. If `focus-track` isn't in `/usr/bin`, put its full path in the file.
2. Add `{ "id": "focus", "type": "qml" }` to `bar.layout.right` in `~/.config/omarchy/shell.json`.
3. `omarchy restart shell`

## Uninstall

```sh
systemctl --user disable --now focus-track
rm ~/.local/bin/focus-track ~/.config/systemd/user/focus-track.service   # Arch package: pacman -R focus-track
rm -r ~/.local/state/focus-track.db* ~/.cache/focus-track ~/.local/share/focus-track ~/.config/focus-track   # your data, backups and config
```
