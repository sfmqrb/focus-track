#!/bin/sh
# focus-track installer: downloads a prebuilt binary from the GitHub release, verifies its SHA-256
# checksum, and installs it for your user only (no sudo, no Rust needed).
#
# Read it before running it:
#   curl -fsSL https://raw.githubusercontent.com/sfmqrb/focus-track/main/install.sh -o install.sh
#   less install.sh && sh install.sh
#
# Options (environment variables):
#   FOCUS_TRACK_VERSION=v0.2.0      install a specific release instead of the latest
#   FOCUS_TRACK_BIN_DIR=~/bin       where the binary goes (default ~/.local/bin)
set -eu

REPO="sfmqrb/focus-track"
VERSION="${FOCUS_TRACK_VERSION:-latest}"
BIN_DIR="${FOCUS_TRACK_BIN_DIR:-$HOME/.local/bin}"
DATA_HOME="${XDG_DATA_HOME:-$HOME/.local/share}"
CONFIG_HOME="${XDG_CONFIG_HOME:-$HOME/.config}"

say() { printf 'focus-track: %s\n' "$*"; }
die() { printf 'focus-track: %s\n' "$*" >&2; exit 1; }
need() { command -v "$1" >/dev/null 2>&1 || die "this installer needs '$1'"; }

case "${1:-}" in
  -h | --help) sed -n '2,11p' "$0" | sed 's/^# \{0,1\}//'; exit 0 ;;
esac

# curl or wget, whichever this machine has
if command -v curl >/dev/null 2>&1; then
  fetch() { curl --proto =https --tlsv1.2 -fsSL "$1"; }
  fetch_to() { curl --proto =https --tlsv1.2 -fsSL -o "$2" "$1"; }
elif command -v wget >/dev/null 2>&1; then
  fetch() { wget -qO- --https-only "$1"; }
  fetch_to() { wget -q --https-only -O "$2" "$1"; }
else
  die "this installer needs 'curl' or 'wget'"
fi
need tar; need install; need mktemp
if command -v sha256sum >/dev/null 2>&1; then
  check_sum() { sha256sum -c --status -; }
elif command -v shasum >/dev/null 2>&1; then
  check_sum() { shasum -a 256 -c --status -; }
else
  die "this installer needs 'sha256sum' (or 'shasum')"
fi

case "$(uname -s)-$(uname -m)" in
  Linux-x86_64 | Linux-amd64) TARGET=x86_64-unknown-linux-musl ;;
  Linux-aarch64 | Linux-arm64) TARGET=aarch64-unknown-linux-musl ;;
  *) die "no prebuilt binary for $(uname -sm). Build it instead (needs Rust): cargo install --locked --git https://github.com/$REPO" ;;
esac

if [ "$VERSION" = latest ]; then
  VERSION=$(fetch "https://api.github.com/repos/$REPO/releases/latest" | sed -n 's/.*"tag_name": *"\([^"]*\)".*/\1/p' | head -n 1) ||
    die "could not reach GitHub to find the latest release"
fi
case "$VERSION" in
  v[0-9]*.[0-9]*.[0-9]*) ;;
  *) die "could not determine a release version (got '$VERSION')" ;;
esac

NAME="focus-track-$VERSION-$TARGET"
URL="https://github.com/$REPO/releases/download/$VERSION"
TMP=$(mktemp -d)
trap 'rm -rf "$TMP"' EXIT INT TERM

say "downloading $NAME"
fetch_to "$URL/$NAME.tar.gz" "$TMP/$NAME.tar.gz" || die "download failed: $URL/$NAME.tar.gz"
fetch_to "$URL/SHA256SUMS" "$TMP/SHA256SUMS" || die "download failed: $URL/SHA256SUMS"
(cd "$TMP" && grep "  $NAME.tar.gz\$" SHA256SUMS | check_sum) || die "checksum mismatch: not installing"
say "checksum OK"

tar -xzf "$TMP/$NAME.tar.gz" -C "$TMP"
mkdir -p "$BIN_DIR"
install -m 755 "$TMP/$NAME/focus-track" "$BIN_DIR/focus-track"
say "installed $BIN_DIR/focus-track"

# shell completions (zsh: `focus-track completions zsh > <a directory on your $fpath>/_focus-track`)
"$BIN_DIR/focus-track" completions bash | install -D -m 644 /dev/stdin "$DATA_HOME/bash-completion/completions/focus-track"
if command -v fish >/dev/null 2>&1; then
  "$BIN_DIR/focus-track" completions fish | install -D -m 644 /dev/stdin "$CONFIG_HOME/fish/completions/focus-track.fish"
fi

case ":$PATH:" in
  *":$BIN_DIR:"*) ;;
  *) say "note: $BIN_DIR is not in your PATH. Add it (for example: export PATH=\"$BIN_DIR:\$PATH\" in your shell profile)" ;;
esac

# how to start recording: a systemd user service where systemd is running, else Hyprland's own autostart
if command -v systemctl >/dev/null 2>&1 && systemctl --user show-environment >/dev/null 2>&1; then
  UNIT="$CONFIG_HOME/systemd/user/focus-track.service"
  if [ ! -e "$UNIT" ]; then
    sed "s|ExecStart=/usr/bin/focus-track daemon|ExecStart=$BIN_DIR/focus-track daemon|" "$TMP/$NAME/extras/focus-track.service" |
      install -D -m 644 /dev/stdin "$UNIT"
    say "wrote $UNIT"
  fi
  START="  systemctl --user daemon-reload
  systemctl --user enable --now focus-track    # start recording now and at every login"
else
  START="  Add this line to your Hyprland config, then log in again (or run it once by hand):
    exec-once = $BIN_DIR/focus-track daemon"
fi

cat <<NEXT

Next:
$START
  focus-track doctor                           # check that everything works
  focus-track dashboard                        # look at your day

Optional, verify the build came from GitHub Actions for this repository:
  gh attestation verify $BIN_DIR/focus-track --repo $REPO
NEXT
