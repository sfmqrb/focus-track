//! The daemon: listens to Hyprland focus events and records them.
//!
//! Privacy rules it enforces:
//! - a browser page title is written only once that exact title is found in the browser's own history
//!   (private/incognito windows never write history). Until then it lives in memory only, and it is
//!   dropped for good after RESOLVE_WINDOW.
//! - nothing leaves the machine: no network access at all.

use crate::config::glob_match;
use crate::store::{self, HEARTBEAT, Store, merged, switches, totals};
use crate::util::{self, cache_dir, day_bounds, expand_home, fmt, hhmm, now, run, today};
use chrono::Timelike;
use rusqlite::{Connection, params};
use std::collections::{HashMap, HashSet};
use std::io::{ErrorKind, Read};
use std::os::unix::ffi::OsStrExt;
use std::os::unix::fs::OpenOptionsExt;
use std::os::unix::net::UnixStream;
use std::path::{Path, PathBuf};
use std::time::Duration;

pub const TICK: f64 = 15.0; // seconds between idle/lock/media checks
pub const RESOLVE_WINDOW: f64 = 300.0; // a browser title must appear in history within this, or it's never stored
pub const SUMMARY_HOUR: u32 = 18; // the end-of-day notification goes out during this hour
pub const PRIVATE: &str = "(private window or not in history: not recorded)";

/// Window class -> history databases (profiles globbed). Omarchy web apps (chrome-*) use chromium's.
pub const BROWSERS: &[(&str, &str)] = &[
    ("chromium", "~/.config/chromium/*/History"),
    ("brave-browser", "~/.config/BraveSoftware/Brave-Browser/*/History"),
    ("google-chrome", "~/.config/google-chrome/*/History"),
    ("firefox", "~/.mozilla/firefox/*/places.sqlite"),
];

const T5: Duration = Duration::from_secs(5);

pub fn is_browser(app: &str) -> bool {
    let a = app.to_lowercase();
    BROWSERS.iter().any(|(b, _)| *b == a) || app.starts_with("chrome-")
}

/// Drop leading spinner/status glyphs (Claude Code's ◐, CLI braille spinners) and the trailing browser name.
pub fn clean_title(title: &str) -> String {
    let t = util::sanitize(title);
    let mut t = t
        .trim_start_matches(|c: char| !(c.is_alphanumeric() || "_~/([".contains(c)))
        .trim()
        .to_string();
    'outer: for browser in ["Chromium", "Google Chrome", "Mozilla Firefox", "Firefox", "Brave"] {
        for dash in ['-', '—', '–'] {
            for space in [' ', '\u{a0}'] {
                let suffix = format!("{space}{dash}{space}{browser}");
                if t.ends_with(&suffix) {
                    t.truncate(t.len() - suffix.len());
                    break 'outer;
                }
            }
        }
    }
    t.chars().take(200).collect()
}

// --- the outside world, behind a trait so the tracker can be tested --------------------------------

pub trait Probe {
    /// (away?, seconds already idle when noticed, locked?)
    fn away(&self) -> (bool, f64, bool);
    /// Is this window class playing audio/video right now?
    fn playing(&self, app: &str) -> bool;
    /// URL for a page title, from the browser's history ("" = not there).
    fn url(&self, app: &str, title: &str) -> String;
}

pub struct System;

impl Probe for System {
    fn away(&self) -> (bool, f64, bool) {
        away_state()
    }
    fn playing(&self, app: &str) -> bool {
        media_playing(app)
    }
    fn url(&self, app: &str, title: &str) -> String {
        lookup_url(app, title)
    }
}

/// Idle/lock from the Omarchy shell when it runs, else logind's hints (hypridle, swayidle and most
/// lockers set them), else assume present.
pub fn away_state() -> (bool, f64, bool) {
    if let Some(s) = shell_idle() {
        return s;
    }
    match logind_hints() {
        Some((idle, locked)) => (idle || locked, 0.0, locked),
        None => (false, 0.0, false),
    }
}

fn shell_idle() -> Option<(bool, f64, bool)> {
    let status: serde_json::Value = serde_json::from_str(&run("omarchy-shell", &["idle", "status"], T5)?).ok()?;
    let locked = run("omarchy-shell", &["lock", "isLocked"], T5).is_some_and(|s| s.trim() == "true");
    let idle = status.get("idle").and_then(|v| v.as_bool()).unwrap_or(false);
    // idle is reported after the first timeout; backdate by it to when input actually stopped
    let idle_for = if idle {
        ["screensaver", "lock"]
            .iter()
            .filter_map(|k| status.get(*k).and_then(|v| v.as_f64()))
            .filter(|v| *v > 0.0)
            .fold(None, |m: Option<f64>, v| Some(m.map_or(v, |m| m.min(v))))
            .unwrap_or(0.0)
    } else {
        0.0
    };
    Some((locked || idle, idle_for, locked))
}

pub fn logind_hints() -> Option<(bool, bool)> {
    let session = std::env::var("XDG_SESSION_ID").unwrap_or_else(|_| "auto".into());
    let out = run("loginctl", &["show-session", &session, "-p", "IdleHint", "-p", "LockedHint"], T5)?;
    let hints: HashMap<&str, &str> = out.lines().filter_map(|l| l.split_once('=')).collect();
    let idle = *hints.get("IdleHint")?;
    Some((idle == "yes", hints.get("LockedHint") == Some(&"yes")))
}

pub fn idle_source() -> Option<&'static str> {
    if shell_idle().is_some() {
        Some("omarchy-shell")
    } else if logind_hints().is_some() {
        Some("logind")
    } else {
        None
    }
}

/// Playing (not paused) audio streams as sets of lowercase names; None without pactl.
pub fn audio_streams() -> Option<Vec<HashSet<String>>> {
    let out = run("pactl", &["-f", "json", "list", "sink-inputs"], Duration::from_secs(3))?;
    let streams: Vec<serde_json::Value> = serde_json::from_str(if out.trim().is_empty() { "[]" } else { &out }).ok()?;
    let keys = ["application.name", "application.process.binary", "application.id", "node.name"];
    Some(
        streams
            .iter()
            .filter(|s| !s.get("corked").and_then(|c| c.as_bool()).unwrap_or(false))
            .map(|s| {
                keys.iter()
                    .filter_map(|k| s.get("properties").and_then(|p| p.get(*k)).and_then(|v| v.as_str()))
                    .map(str::to_lowercase)
                    .filter(|n| !n.is_empty())
                    .collect()
            })
            .collect(),
    )
}

/// Paused players cork their stream, so this means "playing", not "open".
/// ponytail: matched by name, so two windows of one app are indistinguishable.
pub fn media_playing(app: &str) -> bool {
    if app.is_empty() {
        return false;
    }
    let Some(streams) = audio_streams() else { return false };
    let a = app.to_lowercase();
    let last = a.rsplit('.').next().unwrap_or(&a).to_string();
    let aliases: Vec<String> = [
        a.clone(),
        last,
        a.trim_end_matches("-browser").to_string(),
        if a.starts_with("chrome-") {
            "chromium".into()
        } else {
            String::new()
        },
    ]
    .into_iter()
    .filter(|x| x.chars().count() >= 3)
    .collect();
    streams
        .iter()
        .flatten()
        .any(|n| aliases.iter().any(|x| x.contains(n.as_str()) || n.contains(x.as_str())))
}

fn active_window() -> (String, String, String) {
    let v: serde_json::Value = run("hyprctl", &["activewindow", "-j"], T5)
        .and_then(|s| serde_json::from_str(&s).ok())
        .unwrap_or_default();
    let s = |k: &str| v.get(k).and_then(|x| x.as_str()).unwrap_or("").to_string();
    (
        s("class"),
        clean_title(&s("title")),
        s("address").trim_start_matches("0x").to_string(),
    )
}

fn hypr_socket() -> Option<PathBuf> {
    let base = PathBuf::from(std::env::var_os("XDG_RUNTIME_DIR")?).join("hypr");
    if let Some(sig) = std::env::var_os("HYPRLAND_INSTANCE_SIGNATURE") {
        let p = base.join(sig).join(".socket2.sock");
        if p.exists() {
            return Some(p);
        }
    }
    // after a Hyprland restart the signature changes (and a systemd service may not have it): newest instance
    std::fs::read_dir(&base)
        .ok()?
        .flatten()
        .map(|e| e.path().join(".socket2.sock"))
        .filter(|p| p.exists())
        .max_by_key(|p| p.metadata().and_then(|m| m.modified()).ok())
}

// --- browser history ---------------------------------------------------------------------------------

/// Expand `~` and `*`/`?` path components (hidden entries only match patterns that start with a dot).
pub fn expand(pattern: &str) -> Vec<PathBuf> {
    let path = expand_home(pattern);
    let mut acc = vec![PathBuf::from("/")];
    for comp in path.components().skip(1) {
        let c = comp.as_os_str().to_string_lossy().to_string();
        let mut next = Vec::new();
        for base in &acc {
            if c.contains(['*', '?']) {
                for e in std::fs::read_dir(base).into_iter().flatten().flatten() {
                    let name = e.file_name().to_string_lossy().to_string();
                    if (!name.starts_with('.') || c.starts_with('.')) && glob_match(&c, &name) {
                        next.push(base.join(name));
                    }
                }
            } else if base.join(&c).exists() {
                next.push(base.join(&c));
            }
        }
        acc = next;
    }
    acc.sort();
    acc
}

fn fnv(p: &Path) -> u64 {
    p.as_os_str()
        .as_bytes()
        .iter()
        .fold(0xcbf2_9ce4_8422_2325, |h, b| (h ^ u64::from(*b)).wrapping_mul(0x100_0000_01b3))
}

/// Private copy of a browser's history database (browsers keep it locked), refreshed when it changes.
/// The copy is full browsing history, so it lives in a 0700 directory as 0600 files.
fn snapshot(path: &Path) -> Option<PathBuf> {
    let dir = cache_dir();
    store::private_dir(&dir);
    let dest = dir.join(format!("{:016x}.sqlite", fnv(path)));
    let with = |p: &Path, suffix: &str| {
        let mut s = p.as_os_str().to_owned();
        s.push(suffix);
        PathBuf::from(s)
    };
    let mtime = |p: &Path| p.metadata().and_then(|m| m.modified()).ok();
    let mut changed = false;
    for suffix in ["", "-wal"] {
        let (src, dst) = (with(path, suffix), with(&dest, suffix));
        if !src.exists() {
            changed |= std::fs::remove_file(&dst).is_ok();
            continue;
        }
        if matches!((mtime(&src), mtime(&dst)), (Some(a), Some(b)) if a <= b) {
            continue;
        }
        let mut input = std::fs::File::open(&src).ok()?;
        let mut output = std::fs::OpenOptions::new()
            .create(true)
            .write(true)
            .truncate(true)
            .mode(0o600)
            .open(&dst)
            .ok()?;
        std::io::copy(&mut input, &mut output).ok()?;
        changed = true;
    }
    if changed {
        let _ = std::fs::remove_file(with(&dest, "-shm"));
    }
    Some(dest)
}

/// "(3) Inbox" is stored in history as "Inbox".
fn strip_count(t: &str) -> Option<&str> {
    let rest = t.strip_prefix('(')?;
    let close = rest.find(')')?;
    (close > 0 && rest[..close].chars().all(|c| c.is_ascii_digit())).then(|| rest[close + 1..].trim_start())
}

/// URL of the most recent history entry with this exact page title, or "" (private windows never have one).
pub fn lookup_url(app: &str, title: &str) -> String {
    if title.is_empty() || !is_browser(app) {
        return String::new();
    }
    let key = if app.starts_with("chrome-") {
        "chromium".to_string()
    } else {
        app.to_lowercase()
    };
    let Some((_, pattern)) = BROWSERS.iter().find(|(b, _)| *b == key) else {
        return String::new();
    };
    let candidates: Vec<&str> = std::iter::once(title).chain(strip_count(title)).collect();
    for path in expand(pattern) {
        // ponytail: first profile with a match wins
        let sql = if path.ends_with("places.sqlite") {
            "SELECT url FROM moz_places WHERE title = ?1 ORDER BY last_visit_date DESC LIMIT 1"
        } else {
            "SELECT url FROM urls WHERE title = ?1 ORDER BY last_visit_time DESC LIMIT 1"
        };
        let Some(copy) = snapshot(&path) else { continue };
        let Ok(con) = Connection::open(&copy) else { continue };
        for t in &candidates {
            if let Ok(url) = con.query_row(sql, [t], |r| r.get::<_, String>(0)) {
                return util::sanitize(&url);
            }
        }
    }
    String::new()
}

/// Rows never checked against history (older data, or a daemon that died mid-check): keep a browser
/// title only if history has it, otherwise erase it.
pub fn resolve_backlog(con: &Connection, probe: &dyn Probe) {
    let Ok(mut stmt) = con.prepare("SELECT DISTINCT app, COALESCE(title, '') FROM focus WHERE url IS NULL") else {
        return;
    };
    let pairs: Vec<(String, String)> = stmt
        .query_map([], |r| Ok((r.get(0)?, r.get(1)?)))
        .map(|it| it.flatten().collect())
        .unwrap_or_default();
    for (app, title) in pairs {
        let url = probe.url(&app, &title);
        let sql = if !url.is_empty() || !is_browser(&app) {
            "UPDATE focus SET url = ?1 WHERE url IS NULL AND app = ?2 AND COALESCE(title, '') = ?3"
        } else {
            "UPDATE focus SET title = '', url = ?1 WHERE url IS NULL AND app = ?2 AND COALESCE(title, '') = ?3"
        };
        let _ = con.execute(sql, params![url, app, title]);
    }
}

// --- the tracker -------------------------------------------------------------------------------------

type What = (String, String, String); // app, title, mode

pub struct Tracker<'a, P: Probe> {
    con: &'a Connection,
    probe: P,
    pub focused: String,
    pub title: String,
    pub addr: String,
    pub mode: String,
    pub away: bool,
    logged: Option<What>,
    logged_at: f64,
    pub last_tick: f64,
    urls: HashMap<(String, String), String>, // confirmed page titles
    pending: Vec<(i64, What, f64)>,          // rows whose title waits for history
    pub summary: bool,
    pub backups: bool,
    backup_tried: f64,
}

impl<'a, P: Probe> Tracker<'a, P> {
    pub fn new(con: &'a Connection, probe: P) -> Self {
        Tracker {
            con,
            probe,
            focused: String::new(),
            title: String::new(),
            addr: String::new(),
            mode: String::new(),
            away: false,
            logged: None,
            logged_at: 0.0,
            last_tick: now(),
            urls: HashMap::new(),
            pending: Vec::new(),
            summary: false,
            backups: false,
            backup_tried: 0.0,
        }
    }

    fn log(&mut self, what: What, ts: Option<f64>) {
        let ts = ts.unwrap_or_else(now).max(self.logged_at); // never write out of order
        let (app, title, mode) = &what;
        // None = unconfirmed browser title: it stays in memory only (could be a private window)
        let url = if !title.is_empty() && is_browser(app) {
            self.urls.get(&(app.clone(), title.clone())).cloned()
        } else {
            Some(String::new())
        };
        let stored_title = if url.is_some() { title.as_str() } else { "" };
        if self
            .con
            .execute(
                "INSERT INTO focus (ts, app, title, url, mode) VALUES (?1, ?2, ?3, ?4, ?5)",
                params![ts, app, stored_title, url, mode],
            )
            .is_ok()
            && url.is_none()
        {
            self.pending.push((self.con.last_insert_rowid(), what.clone(), now()));
        }
        self.logged = Some(what);
        self.logged_at = ts;
    }

    pub fn resolve(&mut self) {
        for (rowid, what, since) in std::mem::take(&mut self.pending) {
            let key = (what.0.clone(), what.1.clone());
            let url = match self.urls.get(&key) {
                Some(u) => u.clone(),
                None => self.probe.url(&what.0, &what.1),
            };
            if !url.is_empty() {
                let _ = self.con.execute(
                    "UPDATE focus SET title = ?1, url = ?2 WHERE rowid = ?3",
                    params![what.1, url, rowid],
                );
                self.urls.insert(key, url);
            } else if now() - since < RESOLVE_WINDOW {
                self.pending.push((rowid, what, since));
            } else {
                // never showed up in history: private window (or not a real page). The title is gone for good.
                let _ = self.con.execute("UPDATE focus SET url = '' WHERE rowid = ?1", params![rowid]);
            }
        }
    }

    pub fn sync(&mut self, ts: Option<f64>) {
        let what = if self.away {
            Default::default()
        } else {
            (self.focused.clone(), self.title.clone(), self.mode.clone())
        };
        if self.logged.as_ref() != Some(&what) {
            self.log(what, ts);
        }
    }

    pub fn event(&mut self, line: &str) {
        let (name, data) = line.split_once(">>").unwrap_or((line, ""));
        match name {
            // also re-sent when the focused window's title changes
            "activewindow" => {
                let (app, title) = data.split_once(',').unwrap_or((data, ""));
                if app != self.focused {
                    self.mode = if self.probe.playing(app) { "watch".into() } else { String::new() };
                }
                self.focused = app.to_string();
                self.title = clean_title(title);
                self.sync(None);
            }
            "activewindowv2" => self.addr = data.to_string(),
            "windowtitlev2" => {
                let (addr, title) = data.split_once(',').unwrap_or((data, ""));
                if addr == self.addr {
                    self.title = clean_title(title);
                    self.sync(None);
                }
            }
            _ => {}
        }
    }

    pub fn tick(&mut self) {
        let t = now();
        if t - self.last_tick > 4.0 * TICK {
            let at = self.last_tick; // we were asleep (suspend): away since the last tick
            self.log(Default::default(), Some(at));
        }
        self.last_tick = t;
        let (away, idle_for, locked) = self.probe.away();
        self.mode = if !locked && self.probe.playing(&self.focused) {
            "watch".into()
        } else {
            String::new()
        };
        self.away = away && self.mode.is_empty(); // idle but the focused window is playing: watching, not away
        self.sync(if self.away { Some(t - idle_for) } else { None }); // idle is noticed late: backdate
        if !self.pending.is_empty() {
            self.resolve();
        }
        if let Some(last) = self.logged.clone() {
            if !last.0.is_empty() && t - self.logged_at >= HEARTBEAT {
                self.log(last, None);
            }
        }
        if self.summary && chrono::Local::now().hour() == SUMMARY_HOUR && summary_sent() != today().to_string() {
            if let Ok(st) = Store::open() {
                send_summary(&st);
            }
        }
        // once a day (retried hourly if it fails): a verified copy of the database
        if self.backups && t - self.backup_tried >= 3600.0 && crate::backup::due(&crate::backup::dir()) {
            self.backup_tried = t;
            if let Err(e) = crate::backup::make(self.con, &crate::backup::dir(), today()) {
                eprintln!("focus-track: backup failed: {e:#}");
            }
        }
    }
}

fn summary_path() -> PathBuf {
    let mut p = store::db_path().into_os_string();
    p.push(".summary");
    PathBuf::from(p)
}

fn summary_sent() -> String {
    std::fs::read_to_string(summary_path()).unwrap_or_default().trim().to_string()
}

/// End-of-day notification: top apps, longest streak, switches.
pub fn send_summary(st: &Store) {
    let (start, end) = day_bounds(today());
    let sp = st.by_group(start, end);
    let t = totals(&sp);
    let mut body = t
        .iter()
        .take(3)
        .map(|(a, s)| format!("{a} {}", fmt(*s)))
        .collect::<Vec<_>>()
        .join(" · ");
    if body.is_empty() {
        body = "nothing tracked".into();
    }
    if let Some((s, e, app)) = merged(&sp).into_iter().max_by(|a, b| (a.1 - a.0).total_cmp(&(b.1 - b.0))) {
        body += &format!(
            "\nLongest streak: {} in {app} ({})\n{} app switches",
            fmt(e - s),
            hhmm(s),
            switches(&sp)
        );
    }
    let title = format!("Focus today · {}", fmt(store::sum(&t)));
    let _ = run("notify-send", &["-a", "focus-track", &title, &body], T5);
    let _ = std::fs::write(summary_path(), today().to_string());
}

pub fn daemon() -> anyhow::Result<()> {
    let Some(_lock) = store::try_lock() else {
        anyhow::bail!("already running")
    };
    let st = Store::open()?;
    resolve_backlog(&st.con, &System);
    let mut t = Tracker::new(&st.con, System);
    t.summary = true;
    t.backups = true;
    loop {
        if let Some(sock) = hypr_socket().and_then(|p| UnixStream::connect(p).ok()) {
            let _ = sock.set_read_timeout(Some(Duration::from_secs_f64(TICK)));
            (t.focused, t.title, t.addr) = active_window();
            t.mode = if media_playing(&t.focused) { "watch".into() } else { String::new() };
            t.sync(None);
            let (mut buf, mut chunk) = (Vec::new(), vec![0u8; 65536]);
            loop {
                let n = match (&sock).read(&mut chunk) {
                    Ok(0) => break, // Hyprland closed the socket
                    Ok(n) => Some(n),
                    Err(e) if matches!(e.kind(), ErrorKind::WouldBlock | ErrorKind::TimedOut | ErrorKind::Interrupted) => None,
                    Err(_) => break,
                };
                if now() - t.last_tick >= TICK {
                    t.tick(); // before handling events, so a resume from suspend is noticed first
                }
                if let Some(n) = n {
                    buf.extend_from_slice(&chunk[..n]);
                    while let Some(i) = buf.iter().position(|&b| b == b'\n') {
                        let line: Vec<u8> = buf.drain(..=i).collect();
                        t.event(&String::from_utf8_lossy(&line[..line.len() - 1]));
                    }
                }
            }
        }
        // no Hyprland (yet, or restarting): nothing is focused; retry
        t.focused.clear();
        t.title.clear();
        t.mode.clear();
        t.sync(None);
        std::thread::sleep(Duration::from_secs(2));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::{Cell, RefCell};

    struct Fake {
        away: Cell<(bool, f64, bool)>,
        playing: RefCell<HashSet<String>>,
        history: HashMap<String, String>,
    }

    impl Probe for &Fake {
        fn away(&self) -> (bool, f64, bool) {
            self.away.get()
        }
        fn playing(&self, app: &str) -> bool {
            self.playing.borrow().contains(app)
        }
        fn url(&self, _app: &str, title: &str) -> String {
            self.history.get(title).cloned().unwrap_or_default()
        }
    }

    fn fake() -> Fake {
        Fake {
            away: Cell::new((false, 0.0, false)),
            playing: RefCell::new(HashSet::new()),
            history: [("Cats - YouTube".to_string(), "https://youtube.com/watch?v=cats".to_string())].into(),
        }
    }

    fn db() -> Connection {
        let con = Connection::open_in_memory().unwrap();
        store::migrate(&con).unwrap();
        con
    }

    fn rows(con: &Connection) -> Vec<(String, String, Option<String>, String)> {
        let mut s = con.prepare("SELECT app, title, url, mode FROM focus ORDER BY rowid").unwrap();
        s.query_map([], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?)))
            .unwrap()
            .flatten()
            .collect()
    }

    fn apps(con: &Connection) -> Vec<String> {
        rows(con).into_iter().map(|r| r.0).collect()
    }

    #[test]
    fn titles() {
        assert_eq!(clean_title("◑ Interesting ideas"), "Interesting ideas");
        assert_eq!(clean_title("⠋ npm test"), "npm test");
        assert_eq!(clean_title("Cats - YouTube - Chromium"), "Cats - YouTube");
        assert_eq!(clean_title("~/Work"), "~/Work");
        assert_eq!(clean_title("evil\x1b]8;;x\x07 title"), "evil]8;;x title");
        assert_eq!(strip_count("(3) Inbox"), Some("Inbox"));
        assert_eq!(strip_count("(a) Inbox"), None);
    }

    #[test]
    fn focus_titles_idle_suspend() {
        let (con, f) = (db(), fake());
        let mut t = Tracker::new(&con, &f);
        t.event("activewindow>>kitty,zsh");
        t.event("activewindowv2>>abc");
        t.event("activewindow>>kitty,◐ zsh"); // spinner-only change: no new row
        t.event("windowtitlev2>>zzz,other window"); // not the focused window
        assert_eq!(rows(&con).len(), 1);
        t.event("windowtitlev2>>abc,vim notes.txt");
        assert_eq!(rows(&con).last().unwrap().1, "vim notes.txt");

        f.away.set((true, 150.0, false)); // went idle
        t.tick();
        assert_eq!(apps(&con).last().unwrap(), "");
        f.away.set((false, 0.0, false));
        t.event("activewindow>>firefox,x"); // focus change while away is remembered...
        assert_eq!(apps(&con).last().unwrap(), "");
        t.tick(); // ...and logged when you come back
        assert_eq!(apps(&con).last().unwrap(), "firefox");
        t.last_tick -= 10.0 * TICK; // suspend
        t.tick();
        let a = apps(&con);
        assert_eq!(a[a.len() - 2..], ["".to_string(), "firefox".to_string()]);
    }

    #[test]
    fn watching() {
        let (con, f) = (db(), fake());
        let mut t = Tracker::new(&con, &f);
        t.event("activewindow>>mpv,film");
        f.playing.borrow_mut().insert("mpv".into());
        f.away.set((true, 150.0, false));
        t.tick(); // idle, but mpv plays: counted, flagged
        assert_eq!(
            rows(&con).last().map(|r| (r.0.clone(), r.3.clone())),
            Some(("mpv".into(), "watch".into()))
        );
        f.playing.borrow_mut().clear();
        t.tick(); // paused while idle: away
        assert_eq!(apps(&con).last().unwrap(), "");
        f.playing.borrow_mut().insert("mpv".into());
        f.away.set((true, 0.0, true));
        t.tick(); // locked with a film playing: still away
        assert_eq!(apps(&con).last().unwrap(), "");
    }

    #[test]
    fn private_windows_never_hit_disk() {
        let (con, f) = (db(), fake());
        let mut t = Tracker::new(&con, &f);
        t.event("activewindow>>chromium,Secret stuff - Chromium"); // private window: not in history
        t.event("activewindow>>chromium,Cats - YouTube - Chromium");
        assert!(rows(&con).iter().all(|r| r.1.is_empty() && r.2.is_none())); // no title written yet
        t.resolve();
        let r = rows(&con);
        assert_eq!(
            (r[1].1.as_str(), r[1].2.as_deref()),
            ("Cats - YouTube", Some("https://youtube.com/watch?v=cats"))
        );
        assert_eq!(t.pending.len(), 1);
        for p in &mut t.pending {
            p.2 -= RESOLVE_WINDOW;
        }
        t.resolve();
        let r = rows(&con);
        assert_eq!((r[0].1.as_str(), r[0].2.as_deref()), ("", Some(""))); // dropped for good
        assert!(t.pending.is_empty());
        assert!(!rows(&con).iter().any(|r| r.1.contains("Secret")));
    }

    #[test]
    fn backlog_erases_unconfirmed_titles() {
        let (con, f) = (db(), fake());
        con.execute_batch(
            "INSERT INTO focus (ts, app, title, url) VALUES (1, 'chromium', 'Secret', NULL), (2, 'chromium', 'Cats - YouTube', NULL), (3, 'kitty', 'vim', NULL)",
        )
        .unwrap();
        resolve_backlog(&con, &&f);
        let r = rows(&con);
        assert_eq!((r[0].1.as_str(), r[0].2.as_deref()), ("", Some("")));
        assert_eq!(r[1].2.as_deref(), Some("https://youtube.com/watch?v=cats"));
        assert_eq!((r[2].1.as_str(), r[2].2.as_deref()), ("vim", Some("")));
    }
}
