# Demo

The GIF in the README is recorded from random data, never from anyone's real usage.

```sh
cargo build --release
python3 demo/generate.py /tmp/demo.db 9                        # a random week (generic apps, public sites)
python3 demo/record.py target/release/focus-track /tmp/demo.db /tmp/demo.cast "$(date -d yesterday +%F)"
agg --theme 1a1b26,a9b1d6,15161e,f7768e,9ece6a,e0af68,7aa2f7,ad8ee6,449dab,a9b1d6,414868,ff7a93,b9f27c,ff9e64,7da6ff,bb9af7,0db9d7,c0caf5 \
    --font-family "JetBrainsMono NF,Adwaita Mono" --font-size 15 --line-height 1.2 \
    --idle-time-limit 3.5 --last-frame-duration 3 --fps-cap 20 /tmp/demo.cast demo/demo.gif
```

`record.py` drives the real dashboard through a fixed tour in a pseudo-terminal; [agg](https://github.com/asciinema/agg)
renders the recording. Colors: Omarchy's Tokyo Night theme.
