#!/usr/bin/env python3
"""Fill a throwaway database with a random but realistic week, for screenshots and the README GIF.

    python3 demo/generate.py /tmp/demo.db [seed]

Only generic apps and public websites; nothing here comes from anyone's real data.
"""
import datetime as dt
import random
import sqlite3
import sys

APPS = {  # class: (weight, typical minutes per visit, window titles or pages)
    "com.mitchellh.ghostty": (30, 7, ["nvim src/main.rs", "cargo test", "htop", "git log --oneline", "~/projects/api"]),
    "firefox": (30, 3, [
        ("Rust By Example", "https://doc.rust-lang.org/rust-by-example/"),
        ("crossterm - Rust", "https://docs.rs/crossterm/latest/crossterm/"),
        ("Pull requests · example/api", "https://github.com/example/api/pulls"),
        ("Hyprland Wiki", "https://wiki.hypr.land/"),
        ("Lo-fi beats to code to - YouTube", "https://www.youtube.com/watch?v=jfKfPfyJRdk"),
        ("Hacker News", "https://news.ycombinator.com/"),
        ("r/rust", "https://www.reddit.com/r/rust/"),
        ("Stack Overflow - borrowed value does not live long enough", "https://stackoverflow.com/questions/tagged/rust"),
    ]),
    "code": (14, 6, ["main.rs - api - Visual Studio Code", "README.md - api", "Cargo.toml - api"]),
    "obsidian": (8, 4, ["Weekly plan", "Reading notes", "Ideas"]),
    "org.telegram.desktop": (7, 3, ["Telegram"]),
    "slack": (7, 3, ["#general", "#backend", "Direct messages"]),
    "mpv": (4, 12, ["lecture-04.mkv - mpv"]),
    "spotify": (3, 2, ["Spotify Premium"]),
    "zoom": (2, 12, ["Zoom Meeting"]),
    "org.gnome.Nautilus": (2, 1, ["Downloads"]),
}


def main():
    path = sys.argv[1]
    rng = random.Random(int(sys.argv[2]) if len(sys.argv) > 2 else 7)
    con = sqlite3.connect(path)
    con.execute("CREATE TABLE IF NOT EXISTS focus (ts REAL, app TEXT, title TEXT DEFAULT '', url TEXT, mode TEXT DEFAULT '')")
    con.execute("DELETE FROM focus")
    now = dt.datetime.now()
    classes = list(APPS)
    weights = [APPS[c][0] for c in classes]
    for back in range(13, -1, -1):
        day = (now - dt.timedelta(days=back)).date()
        if back > 1 and day.weekday() >= 5 and rng.random() < 0.6:
            continue  # most weekends off
        t = dt.datetime.combine(day, dt.time(8 + rng.randint(0, 2), rng.randint(0, 59))).timestamp()
        end = dt.datetime.combine(day, dt.time(16 + rng.randint(0, 2), rng.randint(0, 59))).timestamp()
        if back == 0:
            end = min(end, now.timestamp() - 60)
        lunch = dt.datetime.combine(day, dt.time(12, rng.randint(10, 40))).timestamp()
        while t < end:
            if lunch and t >= lunch:
                con.execute("INSERT INTO focus VALUES (?, '', '', '', '')", (t,))
                t += rng.randint(35, 55) * 60
                lunch = None
                continue
            if rng.random() < 0.04:  # a coffee, a walk: away
                con.execute("INSERT INTO focus VALUES (?, '', '', '', '')", (t,))
                t += rng.randint(5, 20) * 60
                continue
            app = rng.choices(classes, weights)[0]
            _, minutes, items = APPS[app]
            item = rng.choice(items)
            title, url = item if isinstance(item, tuple) else (item, "")
            mode = "watch" if app in ("mpv", "zoom") or "YouTube" in title else ""
            con.execute("INSERT INTO focus VALUES (?, ?, ?, ?, ?)", (t, app, title, url, mode))
            length = max(0.2, rng.expovariate(1 / minutes)) * 60
            if length > 280:  # the real daemon re-logs every 5 minutes (heartbeat)
                for k in range(1, int(length // 280) + 1):
                    con.execute("INSERT INTO focus VALUES (?, ?, ?, ?, ?)", (t + k * 280, app, title, url, mode))
            t += length
        con.execute("INSERT INTO focus VALUES (?, '', '', '', '')", (t,))
    con.commit()
    print(path, con.execute("SELECT count(*) FROM focus").fetchone()[0], "rows")


if __name__ == "__main__":
    main()
