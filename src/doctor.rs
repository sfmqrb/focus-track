//! `focus-track doctor`: is tracking working? ✓ fine, ! worth a look, ✗ broken.

use crate::render::{dim, paint, theme};
use crate::store::{CAP, Store, daemon_running, sum, totals};
use crate::track::{FAMILIES, audio_streams, expand, hypr_socket, idle_source};
use crate::util::{day_bounds, fmt, hhmm, local, now, today};
use crate::views::page_key;

pub fn doctor_lines(st: &Store) -> (Vec<String>, bool) {
    let th = theme();
    let (ok, warn, bad) = (paint(&th.app[1], "✓"), paint(&th.app[4], "!"), paint(&th.app[5], "✗"));
    let mut out = Vec::new();
    let mut broken = false;
    let mut say = |mark: &str, text: String, detail: String| {
        broken |= mark == bad;
        out.push(format!(
            "{mark} {text}{}",
            if detail.is_empty() {
                String::new()
            } else {
                dim(&format!("  {detail}"))
            }
        ));
    };
    let t = now();

    if hypr_socket().is_none() {
        say(
            &warn,
            "no Hyprland socket found".into(),
            "focus-track records Hyprland sessions; run it from inside Hyprland (exec-once = focus-track daemon)".into(),
        );
    }
    let running = daemon_running();
    if running {
        say(&ok, "tracker is running".into(), String::new());
    } else {
        say(
            &bad,
            "tracker is NOT running".into(),
            "start it: systemctl --user enable --now focus-track (or run: focus-track daemon)".into(),
        );
    }
    let last: Option<(f64, String)> = st
        .con
        .query_row("SELECT ts, app FROM focus ORDER BY ts DESC LIMIT 1", [], |r| {
            Ok((r.get(0)?, r.get(1)?))
        })
        .ok();
    match &last {
        Some((ts, app)) => {
            let stale = running && !app.is_empty() && t - ts > CAP;
            say(
                if stale { &bad } else { &ok },
                format!("last record {} ago", fmt(t - ts)),
                if stale { "the tracker seems stuck".into() } else { String::new() },
            );
        }
        None => say(&warn, "nothing recorded yet".into(), String::new()),
    }

    // gaps: while something was focused the daemon writes at least every HEARTBEAT, so a longer silence means it was down
    let rows: Vec<(f64, String)> = st
        .con
        .prepare("SELECT ts, app FROM focus WHERE ts >= ?1 ORDER BY ts")
        .and_then(|mut s| s.query_map([t - 7.0 * 86400.0], |r| Ok((r.get(0)?, r.get(1)?)))?.collect())
        .unwrap_or_default();
    let mut gaps: Vec<(f64, f64)> = rows
        .windows(2)
        .filter(|w| !w[0].1.is_empty() && w[1].0 - w[0].0 > CAP)
        .map(|w| (w[0].0 + CAP, w[1].0))
        .collect();
    if let Some((ts, app)) = rows.last() {
        if !app.is_empty() && t - ts > CAP {
            gaps.push((ts + CAP, t));
        }
    }
    let lost: f64 = gaps.iter().map(|(s, e)| e - s).sum();
    gaps.sort_by(|a, b| (b.1 - b.0).total_cmp(&(a.1 - a.0)));
    let worst = gaps
        .iter()
        .take(3)
        .map(|(s, e)| format!("{}–{}", local(*s).format("%a %H:%M"), hhmm(*e)))
        .collect::<Vec<_>>()
        .join("; ");
    say(
        if lost > 1800.0 { &warn } else { &ok },
        format!("untracked while the tracker was down (7 days): {}", fmt(lost)),
        worst,
    );

    let (start, _) = day_bounds(today());
    let focused = sum(&totals(&st.by_group(start, t)));
    let watched = sum(&totals(&st.watching(start, t)));
    let first: Option<f64> = st
        .con
        .query_row("SELECT min(ts) FROM focus WHERE ts >= ?1", [start], |r| r.get(0))
        .ok()
        .flatten();
    if let Some(first) = first {
        let down: f64 = gaps.iter().filter(|g| g.1 > start).map(|(s, e)| e - s.max(start)).sum();
        let away = (t - first - focused - down).max(0.0);
        say(
            &ok,
            format!(
                "today since {}: {} focused ({} watching) · {} away",
                hhmm(first),
                fmt(focused),
                fmt(watched),
                fmt(away)
            ),
            String::new(),
        );
    }

    match idle_source() {
        Some(src) => say(&ok, format!("idle/lock detection: {src}"), String::new()),
        None => say(
            &warn,
            "idle/lock detection: none found".into(),
            "time away from the keyboard counts as focused (needs omarchy-shell or logind idle hints)".into(),
        ),
    }
    match audio_streams() {
        Some(s) => say(
            &ok,
            "watching detection: pactl".into(),
            format!("{} playing stream(s) now", s.len()),
        ),
        None => say(
            &warn,
            "watching detection: off".into(),
            "install libpulse (pactl) to count playing video/calls as watching".into(),
        ),
    }

    for (browser, patterns) in FAMILIES {
        let mut profiles: Vec<_> = patterns.iter().flat_map(|p| expand(p)).collect();
        profiles.dedup();
        if !profiles.is_empty() {
            let readable = profiles.iter().filter(|p| std::fs::File::open(p).is_ok()).count();
            say(
                if readable > 0 { &ok } else { &bad },
                format!("{browser}: {readable}/{} history database(s) readable", profiles.len()),
                String::new(),
            );
        }
    }
    // browsers seen in your data whose history we can't find: their page titles are never stored
    let recent: Vec<String> = st
        .con
        .prepare("SELECT DISTINCT app FROM focus WHERE app != '' AND ts >= ?1")
        .and_then(|mut s| s.query_map([t - 30.0 * 86400.0], |r| r.get::<_, String>(0))?.collect())
        .unwrap_or_default();
    let unknown: Vec<String> = recent
        .into_iter()
        .filter(|a| crate::track::is_browser(a) && !crate::track::history_known(a))
        .collect();
    if !unknown.is_empty() {
        say(
            &warn,
            format!(
                "browser without a known history location: {} (page titles and URLs are not stored)",
                unknown.join(", ")
            ),
            "add it under [browsers] in the config to record its pages (focus-track config)".into(),
        );
    }
    let pages: i64 = st
        .con
        .query_row(
            "SELECT count(DISTINCT title) FROM focus WHERE url != '' AND ts >= ?1",
            [start],
            |r| r.get(0),
        )
        .unwrap_or(0);
    let dropped: f64 = totals(&st.spans(start, t, &page_key))
        .iter()
        .filter(|(k, _)| k.starts_with('\t'))
        .map(|x| x.1)
        .sum();
    say(
        &ok,
        format!("pages today: {pages} recorded · {} private/unmatched (never stored)", fmt(dropped)),
        String::new(),
    );

    let cfg = &st.cfg;
    let path = cfg.path.display().to_string();
    if let Some(e) = &cfg.error {
        say(
            &bad,
            format!("config has an error, using defaults: {}", e.lines().next().unwrap_or(e)),
            path,
        );
    } else if !cfg.exists {
        say(
            &warn,
            "no config yet: names are guessed, no categories".into(),
            "focus-track config".into(),
        );
    } else {
        say(
            &ok,
            format!("config: {} renames, {} categories", cfg.names.len(), cfg.categories.len()),
            path,
        );
        let seen: Vec<String> = st
            .con
            .prepare("SELECT DISTINCT app FROM focus WHERE app != '' AND ts >= ?1")
            .and_then(|mut s| s.query_map([t - 30.0 * 86400.0], |r| r.get(0))?.collect())
            .unwrap_or_default();
        let unused: Vec<&str> = cfg
            .names
            .iter()
            .map(|(p, _)| p.as_str())
            .filter(|p| !seen.iter().any(|a| crate::config::glob_match(p, a)))
            .collect();
        if !unused.is_empty() {
            say(
                &warn,
                format!(
                    "renames that matched nothing in 30 days (typo?): {}",
                    unused.iter().take(5).copied().collect::<Vec<_>>().join(", ")
                ),
                String::new(),
            );
        }
        if !cfg.categories.is_empty() {
            let week = totals(&st.spans(t - 7.0 * 86400.0, t, &|r| {
                if !r.app.is_empty() && cfg.category_of(r.app, r.url) == "other" {
                    cfg.name_of(r.app)
                } else {
                    String::new()
                }
            }));
            if !week.is_empty() {
                let list = week
                    .iter()
                    .take(5)
                    .map(|(a, s)| format!("{a} {}", fmt(*s)))
                    .collect::<Vec<_>>()
                    .join(", ");
                say(
                    if sum(&week) > 3600.0 { &warn } else { &ok },
                    format!("uncategorized this week: {list}"),
                    String::new(),
                );
            }
        }
    }

    let (n, first_ts): (i64, Option<f64>) = st
        .con
        .query_row("SELECT count(*), min(ts) FROM focus", [], |r| Ok((r.get(0)?, r.get(1)?)))
        .unwrap_or((0, None));
    let dir = crate::backup::dir();
    let backups = crate::backup::list(&dir);
    match backups.last() {
        None => say(
            &warn,
            "no backups yet".into(),
            "the recorder makes one daily; or run: focus-track backup".into(),
        ),
        Some((newest, _)) => {
            let age = (today() - *newest).num_days();
            say(
                if age > 2 { &warn } else { &ok },
                format!(
                    "backups: {}, newest {}",
                    backups.len(),
                    if age == 0 { "today".to_string() } else { format!("{age} days ago") }
                ),
                dir.display().to_string(),
            );
        }
    }
    let size = std::fs::metadata(&st.path).map(|m| m.len()).unwrap_or(0);
    let since = first_ts
        .map(|f| format!(", since {}", local(f).format("%d %b %Y")))
        .unwrap_or_default();
    say(
        &ok,
        format!("database: {n} rows, {:.1} MB{since}", size as f64 / 1e6),
        st.path.display().to_string(),
    );
    (out, broken)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::render::{set_tty, strip_ansi};
    use crate::store::test_store;

    #[test]
    fn finds_gaps_and_unknown_browsers() {
        set_tty(true); // shared by all tests; compared with colors stripped
        let st = test_store("");
        let t = now();
        // the recorder wrote code at -2h, then nothing until -100 s: a gap it can't have been running through
        st.con
            .execute(
                "INSERT INTO focus (ts, app) VALUES (?1, 'code'), (?2, 'thorium-browser'), (?3, '')",
                rusqlite::params![t - 7200.0, t - 100.0, t - 50.0],
            )
            .unwrap();
        let (lines, broken) = doctor_lines(&st);
        let text = lines.iter().map(|l| strip_ansi(l)).collect::<Vec<_>>().join("\n");
        assert!(text.contains("untracked while the tracker was down (7 days): 1h48"), "{text}");
        assert!(text.contains("browser without a known history location: thorium-browser"), "{text}");
        assert!(text.contains("database: 3 rows"), "{text}");
        assert!(lines.iter().all(|l| !l.is_empty()));
        let _ = broken; // depends on whether a recorder runs on this machine
    }
}
