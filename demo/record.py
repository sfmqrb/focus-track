#!/usr/bin/env python3
"""Drive the real dashboard through a short tour and save it as an asciinema v2 recording.

    python3 demo/record.py <focus-track binary> <demo.db> <out.cast> [YYYY-MM-DD]

Render it with agg (https://github.com/asciinema/agg). See demo/README.md.
"""
import codecs, fcntl, json, os, select, struct, subprocess, sys, termios, time

W, H = 156, 46
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


UP, DOWN, ESC, ENTER, LEFT, RIGHT, HOME, END = (
    b"\x1b[A", b"\x1b[B", b"\x1b", b"\r", b"\x1b[D", b"\x1b[C", b"\x1b[H", b"\x1b[F",
)
pump(2.6)                                   # the overview: today, streaks, activity, timeline, week, heatmap
key(DOWN), key(DOWN, 0.5), key(DOWN, 0.6)   # move the selection down the app list
key(END, 0.7), key(HOME, 0.7)               # jump to the last / first item
key(DOWN), key(DOWN, 1.0)
key(ENTER, 2.4)                             # open one: its windows and when it was used
key(DOWN), key(DOWN), key(DOWN, 1.2)        # scroll its windows
key(ESC, 1.0)
key(b"4", 2.2)                              # zoom the minute-by-minute timeline
key(ESC, 0.9)
key(b"5", 2.0)                              # zoom the week
key(ESC, 0.9)
key(b"6", 2.2)                              # zoom the heatmap: a month of history
key(ESC, 1.0)
key(b"p", 1.9)                              # pages, with their URLs
key(DOWN, 0.5), key(DOWN, 0.5), key(DOWN, 0.5), key(DOWN, 1.3)
key(ESC, 1.0)
key(b"c", 2.4)                              # group by category
key(b"g", 2.2)                              # activity graph: time per category
key(b"g", 0.6), key(b"c", 0.8)              # back to apps / switching
key(LEFT, 1.3), key(LEFT, 1.0)              # two days back
key(RIGHT, 0.8), key(RIGHT, 1.0)            # back to today
key(b"?", 3.2)                              # help: keys and how to read it
key(ESC, 1.6)                               # end back on the overview, not on the quit screen
os.write(fd, b"q")                          # quit without recording the terminal-clear that follows
try:
    proc.wait(timeout=5)
except subprocess.TimeoutExpired:
    proc.kill()

with open(out, "w") as f:
    f.write(json.dumps({"version": 2, "width": W, "height": H, "title": "focus-track"}) + "\n")
    for e in events:
        f.write(json.dumps(e, ensure_ascii=False) + "\n")
print(out, len(events), "events", f"{events[-1][0]:.1f}s")
