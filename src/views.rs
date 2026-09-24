//! Each view returns the lines that go inside a box of inner width `w`.

use crate::render::{DOT, EMPTY, app_code, braille_colored, color, dim, fit, highlight, legend_lines, link, meter, paint, theme, vlen};
use crate::store::{Group, Row, Span, Store, daemon_running, merged, sum, switches, totals};
use crate::track::{PRIVATE, is_browser, without_count};
use crate::util::{day_bounds, days_before, fmt, hhmm, local, now, rhe, today};
use chrono::{NaiveDate, Timelike};
use std::collections::HashMap;
use unicode_width::UnicodeWidthStr;

pub const TODAY_HEAD: usize = 3; // lines above the app list in today_lines (mouse clicks map rows to apps)

/// Scroll state for one list. When the list doesn't fit, the last line becomes a
/// "↑ above · ↓ more" footer, so every item stays reachable.
#[derive(Default, Debug)]
pub struct Pager {
    pub scroll: usize,
    pub n: usize,
    pub total: usize,
}

impl Pager {
    /// (visible items, footer). `keep` = index that must stay on screen.
    pub fn view<T: Clone>(&mut self, items: &[T], cap: usize, keep: Option<usize>) -> (Vec<T>, Option<String>) {
        self.total = items.len();
        self.n = if items.len() <= cap {
            items.len()
        } else {
            cap.saturating_sub(1).max(1)
        };
        if let Some(k) = keep {
            if k < self.scroll {
                self.scroll = k;
            } else if k >= self.scroll + self.n {
                self.scroll = k + 1 - self.n;
            }
        }
        self.scroll = self.scroll.min(items.len().saturating_sub(self.n));
        let footer = (items.len() > self.n).then(|| {
            let (above, below) = (self.scroll, items.len() - self.scroll - self.n);
            let a = if above > 0 { format!("↑ {above} above  ") } else { String::new() };
            let b = if below > 0 { format!("↓ {below} more") } else { String::new() };
            dim(&(a + &b))
        });
        (items[self.scroll..self.scroll + self.n].to_vec(), footer)
    }

    pub fn move_by(&mut self, delta: i64) {
        let max = self.total.saturating_sub(self.n) as i64;
        self.scroll = (self.scroll as i64).saturating_add(delta).clamp(0, max) as usize;
    }

    pub fn reset(&mut self) {
        self.scroll = 0;
    }
}

pub fn nothing(mut lines: Vec<String>) -> Vec<String> {
    lines.push(dim(
        "nothing tracked on this day  ·  ←/→ to change day, `focus-track doctor` if you expected data",
    ));
    lines
}

pub fn day_label(d: NaiveDate) -> String {
    let t = today();
    let name = if d == t {
        "today".to_string()
    } else if d == days_before(t, 1) {
        "yesterday".to_string()
    } else {
        d.format("%A").to_string().to_lowercase()
    };
    format!("{name} · {}", d.format("%a %d %b"))
}

fn names(t: &[(String, f64)]) -> Vec<String> {
    t.iter().map(|x| x.0.clone()).collect()
}

fn ranks(t: &[(String, f64)]) -> HashMap<String, usize> {
    t.iter().enumerate().map(|(i, x)| (x.0.clone(), i)).collect()
}

fn pct(x: f64) -> String {
    format!("{:>4}", format!("{}%", rhe(x * 100.0) as i64))
}

/// Average seconds per group on earlier days that have data (for today: up to the same time of day).
pub fn baseline(st: &Store, d: NaiveDate, days: i64) -> Option<HashMap<String, f64>> {
    let start = day_bounds(d).0;
    let cutoff = if d == today() { now() - start } else { 86400.0 };
    let got: Vec<Vec<(String, f64)>> = (1..=days)
        .map(|i| {
            let s0 = day_bounds(days_before(d, i)).0;
            totals(&st.by_group(s0, s0 + cutoff))
        })
        .filter(|t| !t.is_empty())
        .collect();
    if got.is_empty() {
        return None;
    }
    let mut avg = HashMap::new();
    for t in &got {
        for (a, s) in t {
            *avg.entry(a.clone()).or_insert(0.0) += s / got.len() as f64;
        }
    }
    Some(avg)
}

/// ▲ well above your usual, ▼ well below, · about the same.
fn trend(now: f64, usual: f64) -> String {
    let th = theme();
    if now < 60.0 && usual < 60.0 {
        return dim("–");
    }
    let ratio = if usual > 0.0 { now / usual } else { 9.0 };
    if ratio > 1.25 {
        paint(&th.app[4], "▲")
    } else if ratio < 0.8 {
        paint(&th.app[3], "▼")
    } else {
        dim("–")
    }
}

/// `cap` = lines for the app list. With a pager the list scrolls (`sel` stays in view);
/// without one it shows the first `cap` and a "+ n more" line.
#[allow(clippy::too_many_arguments)]
pub fn today_lines(
    st: &Store,
    d: NaiveDate,
    w: usize,
    sel: Option<usize>,
    goal: f64,
    cap: usize,
    pager: Option<&mut Pager>,
    filter: &str,
) -> Vec<String> {
    let th = theme();
    let (s0, e0) = day_bounds(d);
    let sp = st.by_group(s0, e0);
    let t = totals(&sp);
    let total = sum(&t);
    // the dashboard shows tracking status in its header; the one-shot CLI view shows it here
    let status = match (&pager, daemon_running()) {
        (Some(_), _) => String::new(),
        (None, true) => paint(&th.app[1], &format!("{DOT} tracking")),
        (None, false) => paint(&th.app[5], &format!("{DOT} not tracking")),
    };
    let mut head = format!("{} {}", paint(&format!("1;{}", th.text), &fmt(total)), dim("focused"));
    if let Some(first) = sp.first() {
        let watched = sum(&totals(&st.watching(s0, e0)));
        let w_part = if watched >= 60.0 {
            format!(" · {} watching", fmt(watched))
        } else {
            String::new()
        };
        head += &dim(&format!("{w_part} · {} switches · since {}", switches(&sp), hhmm(first.0)));
    }
    let done = total >= goal;
    let goal_code = if done { &th.app[1] } else { &th.accent };
    let goal_line = format!(
        "{} {} {} {}",
        dim("goal"),
        meter(total / goal, w.saturating_sub(19).max(5), goal_code),
        fit(&dim(&format!("of {}", fmt(goal))), 8),
        if done { paint(&th.app[1], "✓") } else { " ".into() }
    );
    let mut lines = vec![
        if status.is_empty() {
            fit(&head, w)
        } else {
            fit(&head, w.saturating_sub(16)) + &" ".repeat(16usize.saturating_sub(vlen(&status))) + &status
        },
        goal_line,
        String::new(),
    ];
    if t.is_empty() {
        return nothing(lines);
    }
    let usual = baseline(st, d, 7);
    // scale, colors and column widths come from all apps, so filtering or scrolling doesn't make things jump
    let best = t[0].1;
    let name_w = t.iter().map(|x| x.0.width()).max().unwrap_or(1).min(16);
    let bar_w = w.saturating_sub(name_w + 15).max(5);
    // (rank in the full list, name, seconds), narrowed by the search filter
    let list: Vec<(usize, String, f64)> = t
        .iter()
        .enumerate()
        .filter(|(_, (a, _))| matches(a, filter))
        .map(|(i, (a, s))| (i, a.clone(), *s))
        .collect();
    if list.is_empty() {
        lines.push(dim(&format!("no apps match '{filter}'")));
        return lines;
    }
    let (shown, first, footer) = match pager {
        Some(pg) => {
            let (v, f) = pg.view(&list, cap, sel);
            (v, pg.scroll, f)
        }
        None => {
            let rest: f64 = list[cap.min(list.len())..].iter().map(|x| x.2).sum();
            let n_rest = list.len().saturating_sub(cap);
            let footer = (n_rest > 0).then(|| dim(&format!("+ {n_rest} more · {}", fmt(rest))));
            (list[..cap.min(list.len())].to_vec(), 0, footer)
        }
    };
    for (j, (rank, app, s)) in shown.iter().enumerate() {
        let mark = usual
            .as_ref()
            .map_or_else(|| " ".to_string(), |u| trend(*s, *u.get(app).unwrap_or(&0.0)));
        let row = format!(
            "{} {} {:>5} {} {mark}",
            color(*rank, &fit(app, name_w)),
            meter(s / best, bar_w, &app_code(*rank)),
            fmt(*s),
            dim(&pct(s / total))
        );
        lines.push(if Some(first + j) == sel { highlight(&row, w) } else { row });
    }
    lines.extend(footer);
    lines
}

/// Case-insensitive "contains"; an empty filter matches everything.
pub fn matches(text: &str, filter: &str) -> bool {
    filter.is_empty() || text.to_lowercase().contains(&filter.to_lowercase())
}

pub fn streak_lines(st: &Store, d: NaiveDate, top: usize, w: usize) -> Vec<String> {
    let (s0, e0) = day_bounds(d);
    let sp = st.by_group(s0, e0);
    if sp.is_empty() {
        return nothing(vec![]);
    }
    let rank = ranks(&totals(&sp));
    let blocks = merged(&sp);
    let total: f64 = sp.iter().map(|x| x.1 - x.0).sum();
    let stats = dim(&format!(
        "{:.0} switches/h · avg block {}",
        switches(&sp) as f64 / (total / 3600.0),
        fmt(total / blocks.len() as f64)
    ));
    let mut longest = blocks;
    longest.sort_by(|a, b| (b.1 - b.0).total_cmp(&(a.1 - a.0)));
    longest.truncate(top);
    let name_w = longest.iter().map(|x| x.2.width()).max().unwrap_or(1).min(12);
    let bar_w = w.saturating_sub(name_w + 20).max(5);
    let best = longest[0].1 - longest[0].0;
    let mut lines = vec![stats, String::new()];
    for (s, e, app) in &longest {
        let r = rank[app];
        lines.push(format!(
            "{} {} {} {:>5}",
            dim(&hhmm(*s)),
            color(r, &fit(app, name_w)),
            meter((e - s) / best, bar_w, &app_code(r)),
            fmt(e - s)
        ));
    }
    lines
}

/// Zoomed (cap + pager): the hour rows scroll inside `cap` lines; otherwise every hour is returned.
pub fn timeline_lines(st: &Store, d: NaiveDate, top: usize, w: usize, cap: Option<usize>, pager: Option<&mut Pager>) -> Vec<String> {
    let th = theme();
    let (start, end) = day_bounds(d);
    let sp = st.by_group(start, end);
    if sp.is_empty() {
        return nothing(vec![]);
    }
    let ranked: Vec<String> = names(&totals(&sp)).into_iter().take(top).collect();
    let rank: HashMap<&str, usize> = ranked.iter().enumerate().map(|(i, a)| (a.as_str(), i)).collect();
    let per = [1, 2, 3, 4, 5, 6, 10, 12, 15, 20, 30, 60]
        .into_iter()
        .find(|p| 60 / p + 3 <= w)
        .unwrap_or(60); // minutes per cell
    let row = 60 / per;
    let cw = (w.saturating_sub(3) / row).max(1); // stretch cells to fill the box
    let slots = ((end - start) / 60.0 / per as f64).ceil() as usize; // 23 or 25 hours on DST days
    let mut cells: Vec<Option<&str>> = vec![None; slots.max(24 * row)];
    for (s, e, app) in &sp {
        let lo = ((s - start) / (60.0 * per as f64)) as usize;
        let hi = ((e - start - 1.0) / (60.0 * per as f64)).max(0.0) as usize;
        for c in cells.iter_mut().take(hi + 1).skip(lo) {
            *c = Some(app);
        }
    }
    let mut ruler = vec![' '; row * cw];
    for m in [0, 15, 30, 45] {
        for (k, ch) in format!(":{m:02}").chars().enumerate() {
            if let Some(slot) = ruler.get_mut(m / per * cw + k) {
                *slot = ch;
            }
        }
    }
    let mut lines = vec![dim(&format!("   {}", ruler.iter().collect::<String>()))];
    let hours: Vec<usize> = (0..cells.len() / row)
        .filter(|h| cells[h * row..(h + 1) * row].iter().any(Option::is_some))
        .collect();
    let mut rows: Vec<String> = (hours[0]..=hours[hours.len() - 1])
        .map(|h| {
            let label = if slots == 24 * row {
                h
            } else {
                local(start + (h * 3600) as f64).hour() as usize
            }; // DST days
            dim(&format!("{label:02} "))
                + &cells[h * row..(h + 1) * row]
                    .iter()
                    .map(|a| match a {
                        Some(a) => color(*rank.get(a).unwrap_or(&th.app.len()), &DOT.repeat(cw)),
                        None => paint(&th.border, &EMPTY.repeat(cw)),
                    })
                    .collect::<String>()
        })
        .collect();
    if let (Some(cap), Some(pg)) = (cap, pager) {
        let (v, footer) = pg.view(&rows, cap.saturating_sub(3), None); // ruler, blank line and legend take 3
        rows = v;
        rows.extend(footer);
    }
    lines.extend(rows);
    lines.push(String::new());
    lines.extend(legend_lines(&ranked, w));
    lines
}

pub fn week_lines(st: &Store, d: NaiveDate, days: usize, top: usize, w: usize) -> Vec<String> {
    let th = theme();
    let dates: Vec<NaiveDate> = (0..days as i64).rev().map(|i| days_before(d, i)).collect();
    let per_day: Vec<(NaiveDate, Vec<(String, f64)>)> = dates
        .iter()
        .map(|x| {
            let (a, b) = day_bounds(*x);
            (*x, totals(&st.by_group(a, b)))
        })
        .collect();
    let all: Vec<Span> = per_day
        .iter()
        .flat_map(|(_, t)| t.iter().map(|(a, s)| (0.0, *s, a.clone())))
        .collect();
    let overall = totals(&all);
    let ranked: Vec<String> = names(&overall).into_iter().take(top).collect();
    let rank: HashMap<&str, usize> = ranked.iter().enumerate().map(|(i, a)| (a.as_str(), i)).collect();
    let busiest = per_day.iter().map(|(_, t)| sum(t)).fold(0.0, f64::max);
    let busiest = if busiest > 0.0 { busiest } else { 1.0 };
    let bar_w = w.saturating_sub(14).max(5);
    let mut lines = vec![dim(&format!("{} focused in {days} days", fmt(sum(&overall)))), String::new()];
    for (x, t) in &per_day {
        let mut sorted = t.clone();
        sorted.sort_by_key(|(a, _)| *rank.get(a.as_str()).unwrap_or(&top)); // top apps first, "other" last
        let (mut line, mut cum, mut drawn) = (String::new(), 0.0, 0usize);
        for (app, s) in &sorted {
            cum += s;
            let n = (rhe(cum / busiest * bar_w as f64) as usize).saturating_sub(drawn);
            line += &color(*rank.get(app.as_str()).unwrap_or(&th.app.len()), &"■".repeat(n));
            drawn += n;
        }
        let code = if *x == today() {
            format!("1;{}", th.accent)
        } else {
            th.dim.clone()
        };
        let label = paint(&code, &x.format("%a %d").to_string());
        let total = if t.is_empty() { "–".to_string() } else { fmt(sum(t)) };
        lines.push(format!(
            "{label} {line}{} {total:>5}",
            paint(&th.border, &"■".repeat(bar_w.saturating_sub(drawn)))
        ));
    }
    if !ranked.is_empty() {
        lines.push(String::new());
        let mut names = ranked.clone();
        if overall.len() > top {
            names.push("other".into());
        }
        let mut legend = legend_lines(&names, w);
        if overall.len() > top {
            let last = legend.last_mut().expect("never empty");
            *last = last.replace(&color(names.len() - 1, &format!("{DOT} ")), &dim(&format!("{DOT} "))); // "other" is dim
        }
        lines.extend(legend);
    }
    lines
}

pub fn heat_lines(st: &Store, d: NaiveDate, days: usize, w: usize) -> Vec<String> {
    let th = theme();
    let dates: Vec<NaiveDate> = (0..days as i64).rev().map(|i| days_before(d, i)).collect();
    let mut buckets: HashMap<(NaiveDate, u32), f64> = HashMap::new();
    for (mut s, e, _) in st.by_group(day_bounds(dates[0]).0, day_bounds(d).1) {
        while s < e {
            let lt = local(s);
            let hour_start = s - f64::from(lt.minute() * 60 + lt.second()) - s.fract();
            let next = e.min(hour_start + 3600.0);
            *buckets.entry((lt.date_naive(), lt.hour())).or_insert(0.0) += next - s;
            s = next;
        }
    }
    let cw = if w >= 7 + 48 + 6 { 2 } else { 1 };
    let mut lines = vec![dim(&format!(
        "       {}",
        (0..24)
            .step_by(3)
            .map(|h| format!("{h:<width$}", width = 3 * cw))
            .collect::<String>()
    ))];
    for x in &dates {
        let mins: Vec<f64> = (0..24).map(|h| buckets.get(&(*x, h)).copied().unwrap_or(0.0) / 60.0).collect();
        let cells: String = mins
            .iter()
            .map(|m| {
                let pad = " ".repeat(cw - 1);
                if *m < 1.0 {
                    paint(&th.border, EMPTY) + &pad
                } else {
                    paint(&th.heat[((m / 15.0) as usize).min(3)], DOT) + &pad
                }
            })
            .collect();
        let code = if *x == today() {
            format!("1;{}", th.accent)
        } else {
            th.dim.clone()
        };
        let label = paint(&code, &x.format("%a %d").to_string());
        let total: f64 = mins.iter().sum();
        lines.push(format!(
            "{label} {cells} {:>5}",
            if total > 0.0 { fmt(total * 60.0) } else { "–".into() }
        ));
    }
    let key = th.heat.iter().map(|c| paint(c, DOT)).collect::<Vec<_>>().join(" ");
    lines.push(String::new());
    lines.push(dim("less ") + &paint(&th.border, EMPTY) + " " + &key + &dim(" more   one dot = one hour: <15m <30m <45m 45m+"));
    lines
}

/// btop-style braille graph of the day. mode "switches": how often you jumped between apps
/// (tall = scattered, low = deep focus). mode "apps": time stacked by app, one color each.
pub fn graph_lines(st: &Store, d: NaiveDate, w: usize, rows: usize, mode: &str, only: Option<&str>) -> Vec<String> {
    let th = theme();
    let (s0, e0) = day_bounds(d);
    let mut sp = st.by_group(s0, e0);
    let order = totals(&sp);
    let rank = ranks(&order);
    if let Some(o) = only {
        sp.retain(|x| x.2 == o);
    }
    if sp.is_empty() || w == 0 {
        return nothing(vec![]);
    }
    let first = local(sp[0].0);
    let t0 = sp[0].0 - f64::from(first.minute() * 60 + first.second()) - sp[0].0.fract();
    let last = if d == today() { now() } else { sp[sp.len() - 1].1 };
    let t1 = last.max(t0 + 3600.0);
    let cols = 2 * w;
    let step = (t1 - t0) / cols as f64;
    let k = (rhe(900.0 / step) as usize).max(1); // 15-minute rolling window: hills, not a barcode
    let smooth = |xs: &[f64]| -> Vec<f64> {
        (0..cols)
            .map(|i| xs[(i + 1).saturating_sub(k)..=i].iter().sum::<f64>() / k.min(i + 1) as f64)
            .collect()
    };
    let col = |ts: f64| (((ts - t0) / step).floor().max(0.0) as usize).min(cols - 1);
    let (head, lines) = if mode == "switches" && only.is_none() {
        let mut hits = vec![0.0; cols];
        let m = merged(&sp);
        for pair in m.windows(2) {
            if pair[0].2 != pair[1].2 {
                hits[col(pair[1].0)] += 1.0;
            }
        }
        let rate: Vec<f64> = smooth(&hits).iter().map(|x| x * 3600.0 / step).collect();
        let peak = rate.iter().copied().fold(0.0, f64::max);
        let at = rate.iter().position(|r| *r == peak).unwrap_or(0);
        let head = if peak > 0.0 {
            format!(
                "app switches per hour · peak {peak:.0}/h at {} · tall = scattered, low = deep focus",
                hhmm(t0 + at as f64 * step)
            )
        } else {
            "no app switches · deep focus".to_string()
        };
        let norm: Vec<f64> = rate.iter().map(|r| r / if peak > 0.0 { peak } else { 1.0 }).collect();
        let heat = |r: usize, _c: usize| th.heat[rhe((rows - 1 - r) as f64 / (rows.max(2) - 1) as f64 * 3.0) as usize].clone();
        (dim(&head), braille_colored(&norm, rows, &heat))
    } else {
        let bands: Vec<String> = match only {
            Some(o) => vec![o.to_string()],
            None => names(&order).into_iter().take(6).collect(),
        };
        let band_of: HashMap<&str, usize> = bands.iter().enumerate().map(|(i, a)| (a.as_str(), i)).collect();
        let mut per = vec![vec![0.0; cols]; bands.len() + 1]; // last band: every other app
        for (s, e, app) in &sp {
            let band = &mut per[*band_of.get(app.as_str()).unwrap_or(&bands.len())];
            for (i, slot) in band.iter_mut().enumerate().take(col(e - 1e-6) + 1).skip(col(*s)) {
                let (a, b) = (t0 + i as f64 * step, t0 + (i + 1) as f64 * step);
                *slot += (e.min(b) - s.max(a)).max(0.0) / step;
            }
        }
        let per: Vec<Vec<f64>> = per.iter().map(|p| smooth(p)).collect();
        let total: Vec<f64> = (0..cols).map(|i| per.iter().map(|p| p[i]).sum()).collect();
        let codes: Vec<String> = bands
            .iter()
            .map(|a| app_code(rank[a]))
            .chain(std::iter::once(th.dim.clone()))
            .collect();
        let code_at = |r: usize, c: usize| {
            let i = (2 * c + usize::from(total[(2 * c + 1).min(cols - 1)] > total[(2 * c).min(cols - 1)])).min(cols - 1);
            let center = ((rows - 1 - r) as f64 + 0.5) / rows as f64;
            let (mut cum, mut last) = (0.0, bands.len());
            for (b, p) in per.iter().enumerate() {
                cum += p[i];
                if p[i] > 0.0 {
                    last = b;
                }
                if cum >= center {
                    return codes[b].clone();
                }
            }
            codes[last].clone()
        };
        let head = if only.is_some() {
            dim("when you used it (share of each moment)")
        } else {
            bands
                .iter()
                .map(|a| paint(&app_code(rank[a]), &format!("{DOT} ")) + a)
                .collect::<Vec<_>>()
                .join("  ")
        };
        (head, braille_colored(&total, rows, &code_at))
    };
    let mut axis = vec![' '; w];
    let mut hour = t0;
    while hour < t1 {
        let pos = ((hour - t0) / step / 2.0) as usize;
        if pos + 2 <= w && axis[pos.saturating_sub(1)..(pos + 3).min(w)].iter().all(|c| *c == ' ') {
            for (k, ch) in local(hour).format("%H").to_string().chars().enumerate() {
                axis[pos + k] = ch;
            }
        }
        hour += 3600.0;
    }
    let mut out = vec![head];
    out.extend(lines);
    out.push(dim(&axis.iter().collect::<String>()));
    out
}

/// Key for one app's (or category's) detail view: "window title<TAB>url".
pub fn window_key(st: &Store, group: &str, r: &Row) -> String {
    if r.app.is_empty() || st.group_of(r.app, r.url) != group {
        return String::new();
    }
    let label = if !r.title.is_empty() {
        without_count(r.title)
    } else if is_browser(r.app) {
        PRIVATE.into()
    } else {
        "(no title recorded)".into()
    };
    if st.group.get() == Group::Category {
        format!("{} · {label}\t{}", st.cfg.name_of(r.app), r.url)
    } else {
        format!("{label}\t{}", r.url)
    }
}

pub fn page_key(r: &Row) -> String {
    if !r.url.is_empty() {
        format!("{}\t{}", r.url, without_count(r.title))
    } else if is_browser(r.app) {
        format!("\t{PRIVATE}")
    } else {
        String::new()
    }
}

/// Windows of one app (or category) with their time: ("title<TAB>url", seconds), biggest first.
pub fn window_items(st: &Store, d: NaiveDate, group: &str, filter: &str) -> Vec<(String, f64)> {
    let (s0, e0) = day_bounds(d);
    let mut t = totals(&st.spans(s0, e0, &|r| window_key(st, group, r)));
    t.retain(|(k, _)| matches(&k.replace('\t', " "), filter)); // title and URL
    t
}

/// Where the time in one app (or category) went, by window title; browser pages link to their URL.
#[allow(clippy::too_many_arguments)]
pub fn windows_lines(
    st: &Store,
    d: NaiveDate,
    group: &str,
    w: usize,
    n: usize,
    pager: &mut Pager,
    sel: Option<usize>,
    filter: &str,
) -> Vec<String> {
    let (s0, e0) = day_bounds(d);
    let t = window_items(st, d, group, filter);
    if t.is_empty() {
        return if filter.is_empty() {
            nothing(vec![])
        } else {
            vec![dim(&format!("no windows match '{filter}'"))]
        };
    }
    let ranked = names(&totals(&st.by_group(s0, e0)));
    let code = ranked
        .iter()
        .position(|a| a == group)
        .map_or_else(|| theme().accent.clone(), app_code);
    let tw = (w * 3 / 5).clamp(10, 70);
    let bar_w = w.saturating_sub(tw + 8).max(5);
    let best = t[0].1;
    let (shown, footer) = pager.view(&t, n, sel);
    let first = pager.scroll;
    let mut lines: Vec<String> = shown
        .iter()
        .enumerate()
        .map(|(j, (k, s))| {
            let (title, url) = k.split_once('\t').unwrap_or((k, ""));
            let text = if url.is_empty() {
                title.to_string()
            } else {
                format!("{title}{}", dim(&format!("  {}", crate::config::host(url))))
            };
            let row = format!("{} {} {:>5}", link(url, &fit(&text, tw)), meter(s / best, bar_w, &code), fmt(*s));
            if sel == Some(first + j) { highlight(&row, w) } else { row }
        })
        .collect();
    lines.extend(footer);
    lines
}

/// Browser pages in [start, end) as (url, title, seconds), biggest first, plus the private/unmatched total.
pub fn page_items(st: &Store, start: f64, end: f64, search: &str) -> (Vec<(String, String, f64)>, f64) {
    let t = totals(&st.spans(start, end, &page_key));
    let private: f64 = t.iter().filter(|(k, _)| k.starts_with('\t')).map(|x| x.1).sum();
    let needle = search.to_lowercase();
    let pages = t
        .iter()
        .filter(|(k, _)| !k.starts_with('\t'))
        .filter_map(|(k, s)| k.split_once('\t').map(|(u, ti)| (u.to_string(), ti.to_string(), *s)))
        .filter(|(u, ti, _)| needle.is_empty() || format!("{u}{ti}").to_lowercase().contains(&needle))
        .collect();
    (pages, private)
}

/// Every browser page in [start, end) with time and URL; private time only as a total.
/// `n` = lines for the whole panel (2 go to the header).
#[allow(clippy::too_many_arguments)]
pub fn pages_lines(
    st: &Store,
    start: f64,
    end: f64,
    w: usize,
    n: usize,
    search: &str,
    pager: &mut Pager,
    sel: Option<usize>,
) -> Vec<String> {
    let (pages, private) = page_items(st, start, end, search);
    let matching = if search.is_empty() {
        String::new()
    } else {
        format!(" matching '{search}'")
    };
    let private_note = if private > 0.0 {
        format!(" · {} in private windows / unmatched pages: not recorded", fmt(private))
    } else {
        String::new()
    };
    let head = dim(&format!(
        "{} pages · {}{matching}{private_note}",
        pages.len(),
        fmt(pages.iter().map(|p| p.2).sum())
    ));
    if pages.is_empty() {
        let why = if search.is_empty() {
            "no pages recorded".to_string()
        } else {
            format!("no pages match '{search}'")
        };
        return vec![head, String::new(), dim(&why)];
    }
    let tw = (w.saturating_sub(7) * 11 / 20).max(10);
    let uw = w.saturating_sub(7 + tw).max(5);
    let mut lines = vec![head, String::new()];
    let (shown, footer) = pager.view(&pages, n.saturating_sub(2).max(1), sel);
    let first = pager.scroll;
    for (j, (url, title, s)) in shown.into_iter().enumerate() {
        let pretty = strip_scheme(&url);
        let label = if title.is_empty() { pretty.clone() } else { title };
        let row = format!(
            "{:>5}  {} {}",
            fmt(s),
            link(&url, &fit(&label, tw)),
            dim(&link(&url, &fit(&pretty, uw - 1)))
        );
        lines.push(if sel == Some(first + j) { highlight(&row, w) } else { row });
    }
    lines.extend(footer);
    lines
}

fn strip_scheme(url: &str) -> String {
    let rest = url.split_once("://").map_or(url, |(_, r)| r);
    rest.strip_prefix("www.").unwrap_or(rest).to_string()
}

pub fn app_summary_lines(st: &Store, d: NaiveDate, app: &str) -> Vec<String> {
    let th = theme();
    let (s0, e0) = day_bounds(d);
    let sp = st.by_group(s0, e0);
    let t = totals(&sp);
    let mine: Vec<(f64, f64)> = merged(&sp).into_iter().filter(|x| x.2 == app).map(|x| (x.0, x.1)).collect();
    let s = t.iter().find(|x| x.0 == app).map_or(0.0, |x| x.1);
    let total = sum(&t);
    let longest = mine.iter().map(|(b, e)| e - b).fold(0.0, f64::max);
    let mut lines = vec![format!(
        "{} {}{}",
        paint(&format!("1;{}", th.text), &fmt(s)),
        dim(&format!(
            "· {:.0}% of focused time · {} sessions",
            100.0 * s / if total > 0.0 { total } else { 1.0 },
            mine.len()
        )),
        if mine.is_empty() {
            String::new()
        } else {
            dim(&format!(" · longest {}", fmt(longest)))
        }
    )];
    match baseline(st, d, 7) {
        None => lines.push(dim("no earlier days to compare with yet")),
        Some(u) => {
            let usual = *u.get(app).unwrap_or(&0.0);
            let when = if d == today() { "usual by now:" } else { "usual per day:" };
            lines.push(format!("{} {} {}", dim(when), fmt(usual), trend(s, usual)));
        }
    }
    let watched = totals(&st.watching(s0, e0)).into_iter().find(|x| x.0 == app).map_or(0.0, |x| x.1);
    if watched > 0.0 {
        lines.push(dim(&format!("watching (media playing): {} of it", fmt(watched))));
    }
    if st.group.get() == Group::Category {
        let members = names(&totals(&st.spans(s0, e0, &|r| {
            if !r.app.is_empty() && st.group_of(r.app, r.url) == app {
                st.cfg.name_of(r.app)
            } else {
                String::new()
            }
        })));
        lines.push(dim(&format!(
            "apps: {}",
            members.into_iter().take(12).collect::<Vec<_>>().join(", ")
        )));
    } else {
        let mut classes: Vec<String> = totals(&st.spans(s0, e0, &|r| {
            if !r.app.is_empty() && st.cfg.name_of(r.app) == app {
                r.app.to_string()
            } else {
                String::new()
            }
        }))
        .into_iter()
        .map(|x| x.0)
        .collect();
        classes.sort();
        let cat = classes.first().map_or_else(|| "other".to_string(), |c| st.cfg.category_of(c, ""));
        let cat = if cat == "other" && classes.iter().any(|c| is_browser(c)) {
            "per page, by site".to_string()
        } else {
            cat
        };
        lines.push(dim(&format!("class: {} · category: {cat}", classes.join(", "))));
    }
    lines
}

/// Every window class seen in the last `days` days: shown as, category, time.
pub fn apps_lines(st: &Store, days: i64) -> Vec<String> {
    let start = day_bounds(days_before(today(), days - 1)).0;
    let t = totals(&st.spans(start, now(), &|r| r.app.to_string()));
    if t.is_empty() {
        return nothing(vec![]);
    }
    let cw = t.iter().map(|x| x.0.width()).max().unwrap_or(5).min(34);
    let nw = t.iter().map(|x| st.cfg.name_of(&x.0).width()).max().unwrap_or(8).clamp(8, 20);
    let mut lines = vec![
        dim(&format!(
            "{:<cw$}  {:<nw$}  {:<12}  last {days} days",
            "class", "shown as", "category"
        )),
        String::new(),
    ];
    for (app, s) in &t {
        let cat = st.cfg.category_of(app, "");
        let cat_s = if cat == "other" { dim(&cat) } else { paint(&theme().text, &cat) };
        lines.push(format!(
            "{}  {}  {}  {:>6}",
            fit(app, cw),
            fit(&st.cfg.name_of(app), nw),
            fit(&cat_s, 12),
            fmt(*s)
        ));
    }
    lines.push(String::new());
    lines.push(dim(&format!(
        "rename and categorize in {}  (focus-track config)",
        st.cfg.path.display()
    )));
    lines
}

/// Data for the bar's ring widget: today's shares (categories once any are configured) and tracking state.
pub fn widget_json(st: &Store) -> String {
    let th = theme();
    if !st.cfg.categories.is_empty() {
        st.group.set(Group::Category);
    }
    let tracking = daemon_running();
    let last: Option<String> = st
        .con
        .query_row("SELECT app FROM focus ORDER BY ts DESC LIMIT 1", [], |r| r.get(0))
        .ok();
    let present = tracking && last.is_some_and(|a| !a.is_empty());
    let (s0, e0) = day_bounds(today());
    let t = totals(&st.by_group(s0, e0));
    let total = if t.is_empty() { 1.0 } else { sum(&t) };
    let mut slices: Vec<serde_json::Value> = t
        .iter()
        .take(5)
        .enumerate()
        .map(|(i, (_, s))| serde_json::json!([s / total, th.hex[i]]))
        .collect();
    if t.len() > 5 {
        slices.push(serde_json::json!([sum(&t[5..]) / total, ""])); // "other": drawn in the bar's text color
    }
    let state = if !tracking {
        "not tracking (daemon stopped)"
    } else if present {
        "tracking"
    } else {
        "away"
    };
    let apps: Vec<&str> = t.iter().take(5).map(|x| x.0.as_str()).collect();
    let tip = format!(
        "focus · {state}{}\nclick for dashboard",
        if apps.is_empty() {
            String::new()
        } else {
            format!("\n{}", apps.join(" › "))
        }
    );
    serde_json::json!({"slices": slices, "tracking": tracking, "present": present, "accent": th.accent_hex, "tooltip": tip}).to_string()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::render::strip_ansi;

    #[test]
    fn pager() {
        let items: Vec<usize> = (0..20).collect();
        let mut pg = Pager::default();
        assert_eq!(pg.view(&items, 8, None).0, items[..7]);
        assert_eq!(pg.n, 7);
        assert_eq!(*pg.view(&items, 8, Some(19)).0.last().unwrap(), 19);
        assert_eq!(pg.scroll, 13);
        assert_eq!(pg.view(&items, 8, Some(0)).0[0], 0);
        pg.move_by(999);
        let (_, footer) = pg.view(&items, 8, None);
        assert_eq!(pg.scroll, 13);
        let f = strip_ansi(&footer.unwrap());
        assert!(f.contains("above") && !f.contains("more"));
        assert_eq!(Pager::default().view(&items[..5], 8, None), (items[..5].to_vec(), None));
    }
}
