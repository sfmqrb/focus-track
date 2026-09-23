#!/usr/bin/env python3
"""Drive the real dashboard through a short tour and save it as an asciinema v2 recording.

    python3 demo/record.py <focus-track binary> <demo.db> <out.cast> [YYYY-MM-DD]

Render it with agg (https://github.com/asciinema/agg). See demo/README.md.
"""
import codecs, fcntl, json, os, select, struct, subprocess, sys, termios, time

W, H = 112, 36
binary, db, out = sys.argv[1:4]
day = sys.argv[4:5]  # a full day looks better than a morning
here = os.path.dirname(os.path.abspath(__file__))
env = dict(os.environ, FOCUS_TRACK_DB=db, FOCUS_TRACK_CONFIG=os.path.join(here, "config.toml"), TERM="xterm-256color",
           FOCUS_TRACK_THEME=os.environ.get("FOCUS_TRACK_THEME", "/usr/share/omarchy/themes/tokyo-night/colors.toml"))

lock = open(db + ".lock", "w")  # hold the recorder's lock so the header says "tracking"
fcntl.flock(lock, fcntl.LOCK_EX)

fd, tty = os.openpty()
fcntl.ioctl(tty, termios.TIOCSWINSZ, struct.pack("HHHH", H, W, 0, 0))  # size first, so the very first frame fits


def controlling_tty():
    os.setsid()
    fcntl.ioctl(0, termios.TIOCSCTTY, 0)


proc = subprocess.Popen([binary, "dashboard", *(["-d", day[0]] if day else [])], env=env, stdin=tty, stdout=tty, stderr=tty, preexec_fn=controlling_tty)
os.close(tty)
decode = codecs.getincrementaldecoder("utf-8")("replace")  # characters can be split across reads
events, t0 = [], time.time()


def pump(seconds):
    end = time.time() + seconds
    while time.time() < end:
        if select.select([fd], [], [], 0.02)[0]:
            try:
                data = os.read(fd, 1 << 16)
            except OSError:
                return
            text = decode.decode(data)
            if text:
                events.append([round(time.time() - t0, 3), "o", text])


def key(k, wait=0.7):
    os.write(fd, k)
    pump(wait)


UP, DOWN, ESC, ENTER, LEFT, RIGHT = b"\x1b[A", b"\x1b[B", b"\x1b", b"\r", b"\x1b[D", b"\x1b[C"
pump(2.8)                                   # the overview
key(DOWN), key(DOWN, 1.0)                   # select apps
key(ENTER, 2.6)                             # open one: its windows and when it was used
key(DOWN), key(DOWN, 1.2)
key(ESC, 1.2)
key(b"4", 2.4)                              # zoom the minute-by-minute timeline
key(ESC, 1.0)
key(b"p", 2.0)                              # pages, with their URLs
key(DOWN, 0.5), key(DOWN, 0.5), key(DOWN, 1.4)
key(ESC, 1.0)
key(b"c", 2.8)                              # group by category
key(b"g", 2.2)                              # activity graph: time per app
key(b"g", 0.6), key(b"c", 1.0)
key(LEFT, 1.6)                              # the day before
key(RIGHT, 1.0)
key(b"?", 3.4)                              # help: keys and how to read it
key(ESC, 1.2)
key(b"q", 0.4)
proc.wait(timeout=5)

with open(out, "w") as f:
    f.write(json.dumps({"version": 2, "width": W, "height": H, "title": "focus-track"}) + "\n")
    for e in events:
        f.write(json.dumps(e, ensure_ascii=False) + "\n")
print(out, len(events), "events", f"{events[-1][0]:.1f}s")
