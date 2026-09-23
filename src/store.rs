//! The database, and turning its rows into time spans.
//!
//! One row = "from this moment on, this is what had focus". `app` = "" means away / nothing focused.

use crate::config::Config;
use crate::util::{now, sanitize, state_dir};
use rusqlite::Connection;
use std::cell::Cell;
use std::cmp::Ordering;
use std::collections::HashMap;
use std::fs::{File, OpenOptions};
use std::os::unix::fs::{OpenOptionsExt, PermissionsExt};
use std::os::unix::io::AsRawFd;
use std::path::{Path, PathBuf};
use std::time::Duration;

pub const HEARTBEAT: f64 = 300.0; // the daemon re-logs the focused app this often...
pub const CAP: f64 = 2.0 * HEARTBEAT; // ...so a span longer than this means it wasn't running
pub const BLIP: f64 = 3.0; // focus changes shorter than this don't count as switching

pub type Span = (f64, f64, String);

pub struct Row<'a> {
    pub app: &'a str,
    pub title: &'a str,
    pub url: &'a str,
    pub mode: &'a str,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Group {
    App,
    Category,
}

pub struct Store {
    pub con: Connection,
    pub cfg: Config,
    pub group: Cell<Group>,
    pub path: PathBuf,
}

pub fn db_path() -> PathBuf {
    std::env::var_os("FOCUS_TRACK_DB").map_or_else(|| state_dir().join("focus-track.db"), PathBuf::from)
}

pub fn lock_path() -> PathBuf {
    let mut p = db_path().into_os_string();
    p.push(".lock");
    PathBuf::from(p)
}

pub fn connect(path: &Path) -> rusqlite::Result<Connection> {
    let con = Connection::open(path)?;
    con.busy_timeout(Duration::from_secs(5))?;
    migrate(&con)?;
    Ok(con)
}

pub fn migrate(con: &Connection) -> rusqlite::Result<()> {
    con.execute_batch("CREATE TABLE IF NOT EXISTS focus (ts REAL, app TEXT, title TEXT DEFAULT '', url TEXT, mode TEXT DEFAULT '')")?;
    let cols: Vec<String> = con
        .prepare("PRAGMA table_info(focus)")?
        .query_map([], |r| r.get::<_, String>(1))?
        .collect::<Result<_, _>>()?;
    // Older databases: titles came later; url NULL = not yet checked against browser history; mode 'watch' = media playing.
    for (col, def) in [("title", "TEXT DEFAULT ''"), ("url", "TEXT"), ("mode", "TEXT DEFAULT ''")] {
        if !cols.iter().any(|c| c == col) {
            con.execute_batch(&format!("ALTER TABLE focus ADD COLUMN {col} {def}"))?;
        }
    }
    con.execute_batch("CREATE INDEX IF NOT EXISTS focus_ts ON focus(ts)")
}

/// The database holds window titles and page URLs, so only its owner may read it.
pub fn private_file(path: &Path) {
    if path.exists() {
        let _ = std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o600));
    }
}

pub fn private_dir(path: &Path) {
    let _ = std::fs::create_dir_all(path);
    let _ = std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o700));
}

impl Store {
    pub fn open() -> anyhow::Result<Store> {
        let path = db_path();
        if let Some(dir) = path.parent() {
            std::fs::create_dir_all(dir)?;
        }
        let con = connect(&path)?;
        let _ = con.query_row("PRAGMA journal_mode=WAL", [], |_| Ok(())); // the daemon writes while the dashboard reads
        for suffix in ["", "-wal", "-shm"] {
            let mut p = path.clone().into_os_string();
            p.push(suffix);
            private_file(Path::new(&p));
        }
        Ok(Store {
            con,
            cfg: Config::load(),
            group: Cell::new(Group::App),
            path,
        })
    }

    #[cfg(test)]
    pub fn with(con: Connection, cfg: Config) -> Store {
        Store {
            con,
            cfg,
            group: Cell::new(Group::App),
            path: PathBuf::from(":memory:"),
        }
    }

    /// Spans in [start, end), keyed by `key(row)` ("" = not counted).
    pub fn spans(&self, start: f64, end: f64, key: &dyn Fn(&Row) -> String) -> Vec<Span> {
        const COLS: &str = "ts, app, COALESCE(title, ''), COALESCE(url, ''), COALESCE(mode, '')";
        let mut rows: Vec<(f64, String)> = Vec::new();
        let mut take = |sql: String, params: &[f64]| {
            let Ok(mut stmt) = self.con.prepare_cached(&sql) else { return };
            let it = stmt.query_map(rusqlite::params_from_iter(params.iter()), |r| {
                Ok((
                    r.get::<_, f64>(0)?,
                    r.get::<_, String>(1)?,
                    r.get::<_, String>(2)?,
                    r.get::<_, String>(3)?,
                    r.get::<_, String>(4)?,
                ))
            });
            if let Ok(it) = it {
                for (ts, app, title, url, mode) in it.flatten() {
                    let (app, title, url) = (sanitize(&app), sanitize(&title), sanitize(&url));
                    rows.push((
                        ts,
                        key(&Row {
                            app: &app,
                            title: &title,
                            url: &url,
                            mode: &mode,
                        }),
                    ));
                }
            }
        };
        take(format!("SELECT {COLS} FROM focus WHERE ts < ?1 ORDER BY ts DESC LIMIT 1"), &[start]);
        take(
            format!("SELECT {COLS} FROM focus WHERE ts >= ?1 AND ts < ?2 ORDER BY ts"),
            &[start, end],
        );
        let after: Option<f64> = self
            .con
            .query_row("SELECT min(ts) FROM focus WHERE ts >= ?1", [end], |r| r.get(0))
            .ok()
            .flatten();
        clip(&rows, start, end, after.unwrap_or_else(now))
    }

    /// What every view groups by: the app's display name, or its category.
    pub fn group_of(&self, app: &str, url: &str) -> String {
        match self.group.get() {
            Group::Category => self.cfg.category_of(app, url),
            Group::App => self.cfg.name_of(app),
        }
    }

    pub fn by_group(&self, start: f64, end: f64) -> Vec<Span> {
        self.spans(start, end, &|r| {
            if r.app.is_empty() {
                String::new()
            } else {
                self.group_of(r.app, r.url)
            }
        })
    }

    /// Time with media playing in the focused app, by group.
    pub fn watching(&self, start: f64, end: f64) -> Vec<Span> {
        self.spans(start, end, &|r| {
            if !r.app.is_empty() && r.mode == "watch" {
                self.group_of(r.app, r.url)
            } else {
                String::new()
            }
        })
    }
}

/// rows: [(ts, key)] sorted, may start before `start` -> spans clipped to [start, end).
pub fn clip(rows: &[(f64, String)], start: f64, end: f64, now: f64) -> Vec<Span> {
    let mut out = Vec::new();
    for (i, (ts, key)) in rows.iter().enumerate() {
        let next = rows.get(i + 1).map_or(now, |r| r.0);
        let (s, e) = (ts.max(start), next.min(ts + CAP).min(end).min(now));
        if !key.is_empty() && e > s {
            out.push((s, e, key.clone()));
        }
    }
    out
}

/// Join back-to-back spans of the same key (heartbeat and title rows split them). A focus blip
/// shorter than BLIP (the mouse crossing a window) is folded into the span before it.
pub fn merged(sp: &[Span]) -> Vec<Span> {
    let mut out: Vec<Span> = Vec::new();
    for (s, e, k) in sp {
        if let Some(last) = out.last_mut() {
            if s - last.1 < 1.0 && (last.2 == *k || e - s < BLIP) {
                last.1 = *e;
                continue;
            }
        }
        out.push((*s, *e, k.clone()));
    }
    out
}

/// Seconds per key, biggest first (ties keep first-seen order).
pub fn totals(sp: &[Span]) -> Vec<(String, f64)> {
    let mut idx: HashMap<&str, usize> = HashMap::new();
    let mut v: Vec<(String, f64)> = Vec::new();
    for (s, e, k) in sp {
        match idx.get(k.as_str()) {
            Some(&i) => v[i].1 += e - s,
            None => {
                idx.insert(k, v.len());
                v.push((k.clone(), e - s));
            }
        }
    }
    v.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(Ordering::Equal));
    v
}

pub fn sum(t: &[(String, f64)]) -> f64 {
    t.iter().map(|x| x.1).sum()
}

pub fn switches(sp: &[Span]) -> usize {
    merged(sp).windows(2).filter(|w| w[0].2 != w[1].2).count()
}

/// Holds the daemon's single-instance lock while alive.
pub fn try_lock() -> Option<File> {
    let path = lock_path();
    if let Some(dir) = path.parent() {
        let _ = std::fs::create_dir_all(dir);
    }
    let f = OpenOptions::new()
        .create(true)
        .truncate(false)
        .write(true)
        .mode(0o600)
        .open(&path)
        .ok()?;
    (unsafe { libc::flock(f.as_raw_fd(), libc::LOCK_EX | libc::LOCK_NB) } == 0).then_some(f)
}

pub fn daemon_running() -> bool {
    let Ok(f) = OpenOptions::new()
        .create(true)
        .truncate(false)
        .write(true)
        .mode(0o600)
        .open(lock_path())
    else {
        return false;
    };
    let free = unsafe { libc::flock(f.as_raw_fd(), libc::LOCK_EX | libc::LOCK_NB) } == 0;
    !free // dropping `f` releases a lock we just took
}

#[cfg(test)]
pub fn test_store(cfg: &str) -> Store {
    let con = Connection::open_in_memory().unwrap();
    migrate(&con).unwrap();
    Store::with(con, Config::from_str(cfg).unwrap())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rows(v: &[(f64, &str)]) -> Vec<(f64, String)> {
        v.iter().map(|(t, k)| (*t, k.to_string())).collect()
    }

    #[test]
    fn spans_and_totals() {
        let r = rows(&[(0.0, "firefox"), (600.0, "kitty"), (900.0, ""), (1000.0, "firefox")]);
        assert_eq!(
            totals(&clip(&r, 0.0, 1e9, 4600.0)),
            vec![("firefox".into(), 600.0 + CAP), ("kitty".into(), 300.0)]
        );
        assert_eq!(
            clip(&r, 300.0, 700.0, 4600.0),
            vec![(300.0, 600.0, "firefox".into()), (600.0, 700.0, "kitty".into())]
        );
        let hb = rows(&[(0.0, "code"), (300.0, "code"), (600.0, "kitty"), (700.0, "code")]);
        let sp = clip(&hb, 0.0, 900.0, 900.0);
        assert_eq!(
            merged(&sp),
            vec![
                (0.0, 600.0, "code".into()),
                (600.0, 700.0, "kitty".into()),
                (700.0, 900.0, "code".into())
            ]
        );
        assert_eq!(switches(&sp), 2);
        let blips = rows(&[(0.0, "code"), (100.0, "mpv"), (101.0, "code"), (200.0, "mpv")]);
        assert_eq!(
            merged(&clip(&blips, 0.0, 300.0, 300.0)),
            vec![(0.0, 200.0, "code".into()), (200.0, 300.0, "mpv".into())]
        );
    }

    #[test]
    fn store_groups_and_migrates() {
        let st = test_store("[names]\n\"com.mitchellh.ghostty\" = \"Terminal\"\n[categories.work]\napps = [\"Terminal\"]");
        for (ts, app, mode) in [(0.0, "com.mitchellh.ghostty", ""), (100.0, "mpv", "watch"), (200.0, "", "")] {
            st.con
                .execute(
                    "INSERT INTO focus (ts, app, title, url, mode) VALUES (?1, ?2, '', '', ?3)",
                    rusqlite::params![ts, app, mode],
                )
                .unwrap();
        }
        assert_eq!(
            totals(&st.by_group(0.0, 300.0)),
            vec![("Terminal".into(), 100.0), ("mpv".into(), 100.0)]
        );
        assert_eq!(totals(&st.watching(0.0, 300.0)), vec![("mpv".into(), 100.0)]);
        st.group.set(Group::Category);
        assert_eq!(
            totals(&st.by_group(0.0, 300.0)),
            vec![("work".into(), 100.0), ("other".into(), 100.0)]
        );

        // a database from the very first version (no title/url/mode) opens and gains the columns
        let old = Connection::open_in_memory().unwrap();
        old.execute_batch("CREATE TABLE focus (ts REAL, app TEXT); INSERT INTO focus VALUES (1, 'x')")
            .unwrap();
        migrate(&old).unwrap();
        let st = Store::with(old, Config::default());
        assert_eq!(totals(&st.by_group(0.0, 11.0)), vec![("x".into(), 10.0)]);
    }
}
