//! Time, paths and subprocess helpers.

use chrono::{DateTime, Local, LocalResult, NaiveDate, TimeZone};
use std::io::Read;
use std::path::PathBuf;
use std::process::{Command, Stdio};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

pub fn now() -> f64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_secs_f64()).unwrap_or(0.0)
}

pub fn today() -> NaiveDate {
    Local::now().date_naive()
}

/// Local midnight at the start of `d`, as a unix timestamp (handles DST days with no midnight).
pub fn midnight(d: NaiveDate) -> f64 {
    let naive = d.and_hms_opt(0, 0, 0).expect("midnight exists");
    match Local.from_local_datetime(&naive) {
        LocalResult::Single(t) | LocalResult::Ambiguous(t, _) => t.timestamp() as f64,
        LocalResult::None => Local
            .from_local_datetime(&(naive + chrono::Duration::hours(1)))
            .earliest()
            .map_or(0.0, |t| t.timestamp() as f64),
    }
}

pub fn day_bounds(d: NaiveDate) -> (f64, f64) {
    (midnight(d), midnight(d.succ_opt().unwrap_or(d)))
}

pub fn days_before(d: NaiveDate, n: i64) -> NaiveDate {
    d - chrono::Duration::days(n)
}

pub fn local(ts: f64) -> DateTime<Local> {
    Local.timestamp_opt(ts.floor() as i64, 0).single().unwrap_or_else(Local::now)
}

pub fn hhmm(ts: f64) -> String {
    local(ts).format("%H:%M").to_string()
}

/// 59s, 12m, 3h07
pub fn fmt(secs: f64) -> String {
    if secs < 60.0 {
        return format!("{}s", secs.max(0.0) as i64);
    }
    let m = secs as i64 / 60;
    let (h, m) = (m / 60, m % 60);
    if h > 0 { format!("{h}h{m:02}") } else { format!("{m}m") }
}

/// Python-style round (half to even), so bar lengths match the original implementation.
pub fn rhe(x: f64) -> f64 {
    let r = x.round();
    if (x - x.trunc()).abs() == 0.5 { 2.0 * (x / 2.0).round() } else { r }
}

pub fn home() -> PathBuf {
    std::env::var_os("HOME").map_or_else(|| PathBuf::from("/"), PathBuf::from)
}

fn xdg(var: &str, fallback: &str) -> PathBuf {
    std::env::var_os(var)
        .filter(|v| !v.is_empty())
        .map_or_else(|| home().join(fallback), PathBuf::from)
}

pub fn state_dir() -> PathBuf {
    xdg("XDG_STATE_HOME", ".local/state")
}

pub fn cache_dir() -> PathBuf {
    xdg("XDG_CACHE_HOME", ".cache").join("focus-track")
}

pub fn config_dir() -> PathBuf {
    xdg("XDG_CONFIG_HOME", ".config")
}

pub fn expand_home(p: &str) -> PathBuf {
    match p.strip_prefix("~/") {
        Some(rest) => home().join(rest),
        None => PathBuf::from(p),
    }
}

/// Run a helper program and return its stdout, or None if it can't start or takes longer than `timeout`.
/// Programs are looked up on PATH and get no shell, so arguments are never interpreted.
pub fn run(cmd: &str, args: &[&str], timeout: Duration) -> Option<String> {
    let mut child = Command::new(cmd)
        .args(args)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .ok()?;
    let mut out = child.stdout.take()?;
    let reader = std::thread::spawn(move || {
        let mut s = String::new();
        let _ = out.read_to_string(&mut s);
        s
    });
    let start = Instant::now();
    loop {
        match child.try_wait() {
            Ok(Some(_)) => break,
            Ok(None) if start.elapsed() < timeout => std::thread::sleep(Duration::from_millis(10)),
            _ => {
                let _ = child.kill();
                let _ = child.wait();
                return None;
            }
        }
    }
    reader.join().ok()
}

/// Drop control characters (ESC, BEL, newlines...) from text that other programs control, such as
/// window titles and URLs, so it can't inject escape sequences into the terminal.
pub fn sanitize(s: &str) -> String {
    s.chars().filter(|c| !c.is_control()).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn formatting() {
        assert_eq!(fmt(59.9), "59s");
        assert_eq!(fmt(3720.0), "1h02");
        assert_eq!(fmt(600.0), "10m");
        assert_eq!(rhe(2.5), 2.0);
        assert_eq!(rhe(1.5), 2.0);
        assert_eq!(rhe(2.4), 2.0);
        assert_eq!(sanitize("a\x1b]8;;evil\x07b\n"), "a]8;;evilb");
    }

    #[test]
    fn run_with_timeout() {
        assert_eq!(run("echo", &["hi"], Duration::from_secs(2)).as_deref(), Some("hi\n"));
        assert_eq!(run("sleep", &["5"], Duration::from_millis(100)), None);
        assert_eq!(run("definitely-not-a-program-xyz", &[], Duration::from_secs(1)), None);
    }
}
