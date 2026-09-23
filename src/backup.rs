//! Local backups of the database. Nothing in focus-track ever deletes recorded data; backups guard against
//! a lost or corrupted file.
//!
//! One file per day in `~/.local/share/focus-track/backups/`, made with SQLite's `VACUUM INTO` (a consistent
//! snapshot even while the recorder writes), checked, then moved into place. Kept: every day for 14 days,
//! the last of each week for 8 weeks, and the last of each month forever.

use crate::store::{private_dir, private_file};
use crate::util::{home, today};
use anyhow::{Context, ensure};
use chrono::{Datelike, NaiveDate};
use rusqlite::{Connection, OpenFlags};
use std::path::{Path, PathBuf};

const DAILY: i64 = 14;
const WEEKLY: i64 = 56;

pub fn dir() -> PathBuf {
    if let Some(d) = std::env::var_os("FOCUS_TRACK_BACKUP_DIR") {
        return PathBuf::from(d);
    }
    std::env::var_os("XDG_DATA_HOME")
        .filter(|v| !v.is_empty())
        .map_or_else(|| home().join(".local/share"), PathBuf::from)
        .join("focus-track/backups")
}

/// Backups in `dir`, oldest first.
pub fn list(dir: &Path) -> Vec<(NaiveDate, PathBuf)> {
    let mut out: Vec<(NaiveDate, PathBuf)> = std::fs::read_dir(dir)
        .into_iter()
        .flatten()
        .flatten()
        .filter_map(|e| {
            let name = e.file_name().to_string_lossy().to_string();
            let date = name.strip_prefix("focus-track-")?.strip_suffix(".db")?;
            Some((NaiveDate::parse_from_str(date, "%Y-%m-%d").ok()?, e.path()))
        })
        .collect();
    out.sort();
    out
}

/// Is today's backup still missing?
pub fn due(dir: &Path) -> bool {
    list(dir).last().is_none_or(|(d, _)| *d < today())
}

/// Which backups to keep: all recent ones, the last of each week for a while, the last of each month forever.
pub fn keep(dates: &[NaiveDate], today: NaiveDate) -> Vec<bool> {
    let last_in = |d: &NaiveDate, same: &dyn Fn(&NaiveDate) -> bool| !dates.iter().any(|o| o > d && same(o));
    dates
        .iter()
        .enumerate()
        .map(|(i, d)| {
            let age = (today - *d).num_days();
            i + 1 == dates.len() // never the newest
                || age <= DAILY
                || (age <= WEEKLY && last_in(d, &|o| o.iso_week() == d.iso_week()))
                || last_in(d, &|o| o.year() == d.year() && o.month() == d.month())
        })
        .collect()
}

/// Make today's backup of `con` in `dir`, verify it, then prune old ones. Returns the new file.
pub fn make(con: &Connection, dir: &Path, day: NaiveDate) -> anyhow::Result<PathBuf> {
    private_dir(dir);
    let dest = dir.join(format!("focus-track-{day}.db"));
    let tmp = dir.join(format!(".focus-track-{day}.db.tmp"));
    let _ = std::fs::remove_file(&tmp);
    let rows_before: i64 = con.query_row("SELECT count(*) FROM focus", [], |r| r.get(0))?;
    con.execute("VACUUM INTO ?1", [tmp.to_string_lossy()])
        .context("writing the backup")?;
    private_file(&tmp);
    let check = (|| -> anyhow::Result<()> {
        let copy = Connection::open_with_flags(&tmp, OpenFlags::SQLITE_OPEN_READ_ONLY)?;
        let ok: String = copy.query_row("PRAGMA integrity_check", [], |r| r.get(0))?;
        ensure!(ok == "ok", "integrity check failed: {ok}");
        let rows: i64 = copy.query_row("SELECT count(*) FROM focus", [], |r| r.get(0))?;
        ensure!(rows >= rows_before, "backup has {rows} rows, database had {rows_before}");
        Ok(())
    })();
    if let Err(e) = check {
        let _ = std::fs::remove_file(&tmp);
        return Err(e.context("the new backup was bad, so it was discarded (older backups are untouched)"));
    }
    std::fs::rename(&tmp, &dest)?;
    prune(dir, day);
    Ok(dest)
}

pub fn prune(dir: &Path, today: NaiveDate) {
    let all = list(dir);
    let dates: Vec<NaiveDate> = all.iter().map(|x| x.0).collect();
    for ((_, path), keep) in all.iter().zip(keep(&dates, today)) {
        if !keep {
            let _ = std::fs::remove_file(path);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn d(s: &str) -> NaiveDate {
        NaiveDate::parse_from_str(s, "%Y-%m-%d").unwrap()
    }

    #[test]
    fn retention() {
        let today = d("2026-09-23");
        // a backup every day for 200 days
        let dates: Vec<NaiveDate> = (0..200).rev().map(|i| today - chrono::Duration::days(i)).collect();
        let kept: Vec<NaiveDate> = dates.iter().zip(keep(&dates, today)).filter(|x| x.1).map(|x| *x.0).collect();
        assert!(kept.contains(&today));
        for i in 0..=DAILY {
            assert!(
                kept.contains(&(today - chrono::Duration::days(i))),
                "every day of the last two weeks"
            );
        }
        // weekly for 8 weeks: exactly one per ISO week between 15 and 56 days ago
        let weekly: Vec<&NaiveDate> = kept.iter().filter(|x| (15..=WEEKLY).contains(&(today - **x).num_days())).collect();
        let mut weeks: Vec<_> = weekly.iter().map(|x| x.iso_week()).collect();
        weeks.dedup();
        assert!(weeks.len() >= 5 && weekly.len() <= weeks.len() + 2, "{weekly:?}");
        // monthly forever: the oldest months are still there, once each
        for m in ["2026-03", "2026-04", "2026-05", "2026-06"] {
            assert_eq!(kept.iter().filter(|x| x.format("%Y-%m").to_string() == m).count(), 1, "{m}");
        }
        assert!(kept.len() < 45, "kept {}", kept.len());
        assert_eq!(keep(&[d("2020-01-01")], today), vec![true], "a lone old backup is never deleted");
        assert_eq!(keep(&[], today), Vec::<bool>::new());
    }

    #[test]
    fn make_verifies_and_prunes() {
        let dir = std::env::temp_dir().join(format!("focus-track-backup-test-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let con = Connection::open_in_memory().unwrap();
        crate::store::migrate(&con).unwrap();
        con.execute_batch("INSERT INTO focus (ts, app) VALUES (1, 'a'), (2, 'b')").unwrap();
        // an old daily backup that should be pruned, and an old month-end one that stays
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("focus-track-2026-05-10.db"), b"x").unwrap();
        std::fs::write(dir.join("focus-track-2026-05-31.db"), b"x").unwrap();
        let day = d("2026-09-23");
        let file = make(&con, &dir, day).unwrap();
        let copy = Connection::open(&file).unwrap();
        let n: i64 = copy.query_row("SELECT count(*) FROM focus", [], |r| r.get(0)).unwrap();
        assert_eq!(n, 2);
        let names: Vec<String> = list(&dir).iter().map(|x| x.0.to_string()).collect();
        assert_eq!(names, ["2026-05-31", "2026-09-23"]);
        use std::os::unix::fs::PermissionsExt;
        assert_eq!(std::fs::metadata(&file).unwrap().permissions().mode() & 0o777, 0o600);
        assert_eq!(std::fs::metadata(&dir).unwrap().permissions().mode() & 0o777, 0o700);
        assert!(!list(&dir).is_empty() && !due_for(&dir, day));
        make(&con, &dir, day).unwrap(); // same day again: replaces, doesn't pile up
        assert_eq!(list(&dir).len(), 2);
        let _ = std::fs::remove_dir_all(&dir);
    }

    fn due_for(dir: &Path, day: NaiveDate) -> bool {
        list(dir).last().is_none_or(|(d, _)| *d < day)
    }
}
