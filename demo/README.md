# Demo

The GIF in the README is recorded from random data, never from anyone's real usage.

```sh
cargo build --release
python3 demo/generate.py /tmp/demo.db 9                        # four random weeks (generic apps, public sites)
python3 demo/record.py target/release/focus-track /tmp/demo.db /tmp/demo.cast "$(date -d yesterday +%F)"
agg --theme 1a1b26,a9b1d6,15161e,f7768e,9ece6a,e0af68,7aa2f7,ad8ee6,449dab,a9b1d6,414868,ff7a93,b9f27c,ff9e64,7da6ff,bb9af7,0db9d7,c0caf5 \
    --font-family "JetBrainsMono NF,Adwaita Mono" --font-size 13 --line-height 1.2 \
    --idle-time-limit 3.5 --last-frame-duration 3 --fps-cap 20 /tmp/demo.cast demo/demo.gif
```

`generate.py` fills four weeks so the heatmap, week view and trend arrows have real history; seed 9 gives a
balanced day (no single app dominating, several categories). `record.py` drives the real dashboard at 156x46
(every panel visible at once: today, streaks, activity, timeline, week and heatmap) through a longer tour —
overview, moving the selection, an app's details and its windows, zooming the timeline/week/heatmap, the pages
list, grouping by category with the stacked activity graph, day navigation, and help — in a pseudo-terminal;
[agg](https://github.com/asciinema/agg) renders the recording at a smaller font so the GIF stays readable-sized.
Colors: Omarchy's Tokyo Night theme.
