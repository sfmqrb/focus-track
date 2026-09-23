# Install

## Arch Linux (AUR)

```sh
yay -S focus-track     # or: paru -S focus-track
```

Installs `/usr/bin/focus-track`, a systemd user unit, shell completions (bash, zsh, fish) and the Omarchy bar widget
in `/usr/share/focus-track/omarchy/`.

## Prebuilt binary (any Linux, x86_64 or aarch64)

```sh
curl -fsSL https://raw.githubusercontent.com/sfmqrb/focus-track/main/install.sh -o install.sh
less install.sh        # it's short; read it first
sh install.sh
```

It installs to `~/.local/bin` (no sudo), checks the download against the release's `SHA256SUMS`, and adds bash/fish
completions and a systemd user unit (not enabled). Options: `FOCUS_TRACK_VERSION=v0.1.0`, `FOCUS_TRACK_BIN_DIR=~/bin`.

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

The recorder is `focus-track daemon`; run it with your session. With systemd:

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

1. Copy [`extras/omarchy/focus.qml`](../extras/omarchy/focus.qml) (AUR: `/usr/share/focus-track/omarchy/focus.qml`)
   to `~/.config/omarchy/bar/modules/focus.qml`. If `focus-track` isn't in `/usr/bin`, put its full path in the file.
2. Add `{ "id": "focus", "type": "qml" }` to `bar.layout.right` in `~/.config/omarchy/shell.json`.
3. `omarchy restart shell`

## Uninstall

```sh
systemctl --user disable --now focus-track
rm ~/.local/bin/focus-track ~/.config/systemd/user/focus-track.service   # AUR: pacman -R focus-track
rm -r ~/.local/state/focus-track.db* ~/.cache/focus-track ~/.config/focus-track   # your data and config
```
