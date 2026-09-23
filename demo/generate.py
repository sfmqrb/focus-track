#!/usr/bin/env python3
"""Fill a throwaway database with a random but realistic month, for screenshots and the README GIF.

    python3 demo/generate.py /tmp/demo.db [seed]

Only generic apps and public websites; nothing here comes from anyone's real data.
"""
import datetime as dt
import random
import sqlite3
import sys

APPS = {  # class: (weight, typical minutes per visit, window titles or pages)
    "com.mitchellh.ghostty": (28, 7, [
        "nvim src/main.rs", "nvim src/lib.rs", "cargo test", "cargo build --release", "cargo clippy",
        "htop", "git log --oneline", "git diff", "git rebase -i main", "ssh build-server",
        "docker compose up", "make test", "~/projects/api", "~/projects/web",
    ]),
    "firefox": (30, 3, [
        ("Rust By Example", "https://doc.rust-lang.org/rust-by-example/", ""),
        ("The Rust Programming Language", "https://doc.rust-lang.org/book/", ""),
        ("crossterm - Rust", "https://docs.rs/crossterm/latest/crossterm/", ""),
        ("ratatui - Rust", "https://docs.rs/ratatui/latest/ratatui/", ""),
        ("Pull requests · example/api", "https://github.com/example/api/pulls", ""),
        ("Issues · example/web", "https://github.com/example/web/issues", ""),
        ("example/api: a small service", "https://github.com/example/api", ""),
        ("Hyprland Wiki", "https://wiki.hypr.land/", ""),
        ("Array - JavaScript | MDN", "https://developer.mozilla.org/en-US/docs/Web/JavaScript/Reference/Global_Objects/Array", ""),
        ("Rust (programming language) - Wikipedia", "https://en.wikipedia.org/wiki/Rust_(programming_language)", ""),
        ("Git - Wikipedia", "https://en.wikipedia.org/wiki/Git", ""),
        ("Lo-fi beats to code to - YouTube", "https://www.youtube.com/watch?v=jfKfPfyJRdk", "watch"),
        ("Rust Conf 2024 Keynote - YouTube", "https://www.youtube.com/watch?v=example1", "watch"),
        ("Hacker News", "https://news.ycombinator.com/", ""),
        ("r/rust", "https://www.reddit.com/r/rust/", ""),
        ("r/programming", "https://www.reddit.com/r/programming/", ""),
        ("Stack Overflow - borrowed value does not live long enough", "https://stackoverflow.com/questions/tagged/rust", ""),
        ("Stack Overflow - iterator vs into_iter", "https://stackoverflow.com/questions/tagged/rust", ""),
        ("Tech - BBC News", "https://www.bbc.com/news/technology", ""),
    ]),
    "code": (13, 6, ["main.rs - api - Visual Studio Code", "lib.rs - api - Visual Studio Code", "README.md - api", "Cargo.toml - api", "index.ts - web - Visual Studio Code"]),
    "obsidian": (7, 4, ["Weekly plan", "Reading notes", "Ideas", "Meeting notes", "Project roadmap"]),
    "org.telegram.desktop": (6, 3, ["Telegram"]),
    "slack": (7, 3, ["#general", "#backend", "#frontend", "Direct messages", "#announcements"]),
    "discord": (3, 3, ["#rust-lang", "#general", "Voice channel"]),
    "mpv": (4, 14, ["lecture-04-async-rust.mkv - mpv", "conf-talk-error-handling.mkv - mpv", "tutorial-hyprland-setup.mkv - mpv"]),
    "spotify": (3, 2, ["Spotify Premium"]),
    "zoom": (3, 20, ["Daily Standup - Zoom", "Sprint Planning - Zoom", "1:1 with manager - Zoom", "Team Sync - Zoom", "Design Review - Zoom"]),
    "postman": (2, 3, ["api.example.com/v1/users", "api.example.com/v1/orders"]),
    "org.gnome.Nautilus": (2, 1, ["Downloads", "Projects"]),
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
    for back in range(27, -1, -1):  # four weeks: enough history for the heatmap, week view and trend arrows
        day = (now - dt.timedelta(days=back)).date()
        weekday = day.weekday()
        if back > 1 and weekday >= 5 and rng.random() < 0.65:
            continue  # most weekends off
        start_hour = 7 + rng.randint(0, 3)  # some days start early, some start late
        start = dt.datetime.combine(day, dt.time(start_hour, rng.randint(0, 59))).timestamp()
        if weekday == 4:  # a light Friday
            end = dt.datetime.combine(day, dt.time(13 + rng.randint(0, 2), rng.randint(0, 59))).timestamp()
        else:
            end = dt.datetime.combine(day, dt.time(16 + rng.randint(0, 3), rng.randint(0, 59))).timestamp()
        if back == 0:
            end = min(end, now.timestamp() - 60)
        lunch = dt.datetime.combine(day, dt.time(12, rng.randint(10, 40))).timestamp()
        morning_break = dt.datetime.combine(day, dt.time(10, rng.randint(10, 40))).timestamp()
        t = start
        while t < end:
            if lunch and t >= lunch:
                con.execute("INSERT INTO focus VALUES (?, '', '', '', '')", (t,))
                t += rng.randint(35, 55) * 60
                lunch = None
                continue
            if morning_break and t >= morning_break:
                con.execute("INSERT INTO focus VALUES (?, '', '', '', '')", (t,))
                t += rng.randint(5, 12) * 60
                morning_break = None
                continue
            if rng.random() < 0.04:  # a coffee, a walk: away
                con.execute("INSERT INTO focus VALUES (?, '', '', '', '')", (t,))
                t += rng.randint(5, 20) * 60
                continue
            app = rng.choices(classes, weights)[0]
            _, minutes, items = APPS[app]
            item = rng.choice(items)
            title, url, mode = item if isinstance(item, tuple) else (item, "", "")
            if app in ("mpv", "zoom"):
                mode = "watch"
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
