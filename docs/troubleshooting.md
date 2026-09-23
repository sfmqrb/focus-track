# Troubleshooting

Start with `focus-track doctor`. It checks the recorder, idle detection, browsers, backups and config, and says what to fix.

## Install

**`mise ERROR No version is set for shim: cargo`** (or `rustc`): `cargo install` builds from source and needs Rust.
Your version manager has no Rust selected. You don't need it: use the prebuilt binary
(`curl -fsSL https://raw.githubusercontent.com/sfmqrb/focus-track/main/install.sh | sh`). To build from source,
set Rust up first (`mise use -g rust`, or install [rustup](https://rustup.rs)). The options in the README are
alternatives: run one of them, not all.

**`focus-track: command not found`** after installing: the binary is in `~/.local/bin`, which isn't in your `PATH`.
Add `export PATH="$HOME/.local/bin:$PATH"` to your shell profile and open a new terminal.

**The installer says it needs `curl` or `wget`, or `sha256sum`**: install one of them (they're in every distro's basic
tools), or download the release from GitHub by hand.

**No prebuilt binary for your machine**: builds exist for x86_64 and aarch64 Linux. Otherwise build from source.

## Recording

**Nothing is recorded / "no Hyprland socket found"**: the recorder needs a running Hyprland session. Start it from
inside Hyprland (`exec-once = focus-track daemon`), or with the systemd user service after logging in. It waits until
Hyprland is available and writes that to its log (`journalctl --user -u focus-track`).

**I don't use systemd**: skip the service and add `exec-once = focus-track daemon` to your Hyprland config.

**Time is counted while I'm away**: focus-track needs to learn when you're idle or locked. It asks the Omarchy shell, or
logind's idle/lock hints (hypridle and swayidle set them; `focus-track doctor` shows which source it found). If neither
exists, idle time counts as focused.

**Playing video isn't counted as *watching***: it needs `pactl` (PipeWire's or PulseAudio's). Install `libpulse` or
`pipewire-pulse`.

## Browsers

**A browser's pages don't show up** (and `doctor` mentions it): focus-track only records a page title once it finds it
in that browser's own history, which is how private windows are kept out. For a browser it doesn't know, declare its
history location under `[browsers]`, see [Configuration](configuration.md#browsers). Flatpak, Snap and other
`XDG_CONFIG_HOME` layouts of the common browsers are found automatically.

**Web apps** (Omarchy's Discord, WhatsApp, …) appear under their site's name. Rename one with `[names]`.

## Data

Everything is in `~/.local/state/focus-track.db`, backups in `~/.local/share/focus-track/backups/`.
See [Usage](usage.md#backups) to restore, and [SECURITY.md](../SECURITY.md) for what is stored.
