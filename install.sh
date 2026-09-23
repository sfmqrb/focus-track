#!/bin/sh
# focus-track installer: downloads a prebuilt binary from the GitHub release, verifies its SHA-256
# checksum, and installs it for your user only (no sudo).
#
# Read it before running it:
#   curl -fsSL https://raw.githubusercontent.com/sfmqrb/focus-track/main/install.sh -o install.sh
#   less install.sh && sh install.sh
#
# Options (environment variables):
#   FOCUS_TRACK_VERSION=v0.1.0      install a specific release instead of the latest
#   FOCUS_TRACK_BIN_DIR=~/bin       where the binary goes (default ~/.local/bin)
set -eu

REPO="sfmqrb/focus-track"
VERSION="${FOCUS_TRACK_VERSION:-latest}"
BIN_DIR="${FOCUS_TRACK_BIN_DIR:-$HOME/.local/bin}"
CURL="curl --proto =https --tlsv1.2 -fsSL"

say() { printf 'focus-track: %s\n' "$*"; }
die() { printf 'focus-track: %s\n' "$*" >&2; exit 1; }
need() { command -v "$1" >/dev/null 2>&1 || die "this installer needs '$1'"; }

need curl; need tar; need sha256sum; need install

case "$(uname -s)-$(uname -m)" in
  Linux-x86_64) TARGET=x86_64-unknown-linux-musl ;;
  Linux-aarch64 | Linux-arm64) TARGET=aarch64-unknown-linux-musl ;;
  *) die "no prebuilt binary for $(uname -sm); build it instead: cargo install --locked --git https://github.com/$REPO" ;;
esac

if [ "$VERSION" = latest ]; then
  VERSION=$($CURL "https://api.github.com/repos/$REPO/releases/latest" | sed -n 's/.*"tag_name": *"\([^"]*\)".*/\1/p' | head -n 1)
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
$CURL -o "$TMP/$NAME.tar.gz" "$URL/$NAME.tar.gz"
$CURL -o "$TMP/SHA256SUMS" "$URL/SHA256SUMS"
(cd "$TMP" && grep "  $NAME.tar.gz\$" SHA256SUMS | sha256sum -c --status -) || die "checksum mismatch: not installing"
say "checksum OK"

tar -xzf "$TMP/$NAME.tar.gz" -C "$TMP"
mkdir -p "$BIN_DIR"
install -m 755 "$TMP/$NAME/focus-track" "$BIN_DIR/focus-track"
say "installed $BIN_DIR/focus-track"

# shell completions
"$BIN_DIR/focus-track" completions bash | install -D -m 644 /dev/stdin "$HOME/.local/share/bash-completion/completions/focus-track"
if command -v fish >/dev/null 2>&1; then
  "$BIN_DIR/focus-track" completions fish | install -D -m 644 /dev/stdin "$HOME/.config/fish/completions/focus-track.fish"
fi

# systemd user service (not enabled: you decide)
UNIT="$HOME/.config/systemd/user/focus-track.service"
if [ ! -e "$UNIT" ]; then
  sed "s|ExecStart=/usr/bin/focus-track daemon|ExecStart=$BIN_DIR/focus-track daemon|" "$TMP/$NAME/extras/focus-track.service" |
    install -D -m 644 /dev/stdin "$UNIT"
  say "wrote $UNIT"
fi

case ":$PATH:" in
  *":$BIN_DIR:"*) ;;
  *) say "note: $BIN_DIR is not in your PATH" ;;
esac

cat <<NEXT

Next:
  systemctl --user daemon-reload
  systemctl --user enable --now focus-track    # start recording (or add 'focus-track daemon' to your Hyprland autostart)
  focus-track doctor                           # check that everything works
  focus-track dashboard                        # look at your day

Optional, verify the build came from GitHub Actions for this repository:
  gh attestation verify $BIN_DIR/focus-track --repo $REPO
NEXT
