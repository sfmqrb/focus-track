# focus-track

**See where your time goes on Hyprland.** A small recorder notes which window has focus, and a
btop-style dashboard shows your day. Everything stays on your machine.

![focus-track dashboard](demo/demo.gif)

- **Counts only real time**: idle, locked and suspended time is skipped; a playing video or call counts as *watching*.
- **Pages, privately**: time per website with its URL. Private/incognito windows are never recorded.
- **Your categories**: work, media, chat… by rules you write. No AI, no network, ever.

## Install

```sh
yay -S focus-track                                   # Arch (AUR)
curl -fsSL https://raw.githubusercontent.com/sfmqrb/focus-track/main/install.sh | sh   # any Linux
cargo install --locked --git https://github.com/sfmqrb/focus-track                    # from source
```

Then start recording and check it works:

```sh
systemctl --user enable --now focus-track
focus-track doctor
```

## Use

```sh
focus-track dashboard     # the live dashboard (press ? for help)
focus-track               # today at a glance
focus-track week          # the last 7 days
focus-track pages         # websites you visited
```

## More

- [Install](docs/install.md): verifying downloads, autostart without systemd, the Omarchy bar widget, uninstalling
- [Usage](docs/usage.md): every command, dashboard keys, how to read the charts
- [Configuration](docs/configuration.md): names, merging apps, categories
- [Privacy & security](SECURITY.md): exactly what is stored, where, and how to delete it

MIT license.
