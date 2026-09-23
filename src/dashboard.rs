//! The live, btop-style full-screen view.
//!
//! Layout: a header bar (day, status, goal, clock), the panels, and one key bar at the bottom that always
//! shows what the keys do *right now*. Every list works the same way: ↑↓ PgUp PgDn Home End move a reversed
//! selection row, ⏎ opens it, Esc goes back one level, ? shows help.

use crate::render::{DOT, EMPTY, boxed, dim, fit, hints, hstack, paint, theme};
use crate::store::{Group, Store, daemon_running, sum, totals};
use crate::util::{day_bounds, days_before, fmt, today};
use crate::views::{self, Pager, TODAY_HEAD, day_label};
use chrono::NaiveDate;
use crossterm::event::{self, Event, KeyCode, KeyEventKind, KeyModifiers, MouseButton, MouseEventKind};
use std::io::Write;
use std::time::Duration;

pub const PANELS: [&str; 7] = ["today", "streaks", "activity", "timeline", "week", "heatmap", "pages"];
const SUP: [&str; 7] = ["¹", "²", "³", "⁴", "⁵", "⁶", "⁷"];
type Rect = (usize, usize, usize, usize); // x, y, w, h
const CHROME: usize = 2; // header bar + key bar
pub const MIN_W: usize = 60;
pub const MIN_H: usize = 16 + CHROME;

pub struct Ui {
    pub d: NaiveDate,
    pub top: usize,
    pub goal: f64,
    pub sel: usize,
    pub sel_app: Option<String>, // selection follows the app, not the row: rankings shift while you watch
    pub focus: &'static str,
    pub zoom: Option<&'static str>,
    pub detail: Option<String>,
    pub help: bool,
    back: Option<&'static str>, // where Esc from the detail view returns to
    pub graph: &'static str,
    pub apps: Vec<String>,
    rects: Vec<(&'static str, Rect)>,
    pub pg_today: Pager,
    pub pg_timeline: Pager,
    pub pg_pages: Pager,
    pub pg_windows: Pager,
    pub page_sel: usize,
    pub win_sel: usize,
    pub opener: fn(&str) -> bool,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Input {
    Char(char),
    Up,
    Down,
    PgUp,
    PgDn,
    Home,
    End,
    Left,
    Right,
    Tab,
    BackTab,
    Enter,
    Esc,
    Quit,
    Wheel(bool, usize, usize), // (up?, x, y)
    Click(usize, usize),
}

#[derive(Clone, Copy)]
enum Step {
    Up,
    Down,
    PgUp,
    PgDn,
    Home,
    End,
}

/// Only web links are handed to the browser, and never through a shell.
pub fn safe_url(url: &str) -> bool {
    (url.starts_with("https://") || url.starts_with("http://"))
        && url.len() > 8
        && !url.chars().any(|c| c.is_control() || c.is_whitespace())
}

fn open_url(url: &str) -> bool {
    safe_url(url)
        && std::process::Command::new("xdg-open")
            .arg(url)
            .stdin(std::process::Stdio::null())
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .spawn()
            .is_ok()
}

impl Ui {
    pub fn new(d: NaiveDate, top: usize, goal: f64) -> Ui {
        Ui {
            d,
            top,
            goal,
            sel: 0,
            sel_app: None,
            focus: "today",
            zoom: None,
            detail: None,
            help: false,
            back: None,
            graph: "switches",
            apps: vec![],
            rects: vec![],
            pg_today: Pager::default(),
            pg_timeline: Pager::default(),
            pg_pages: Pager::default(),
            pg_windows: Pager::default(),
            page_sel: 0,
            win_sel: 0,
            opener: open_url,
        }
    }

    fn set_day(&mut self, d: NaiveDate) {
        self.d = d.min(today());
        for pg in [&mut self.pg_timeline, &mut self.pg_pages, &mut self.pg_windows] {
            pg.reset();
        }
        self.page_sel = 0;
        self.win_sel = 0;
    }

    fn open(&mut self, i: usize) {
        self.detail = Some(self.apps[i].clone());
        self.back = self.zoom;
        self.zoom = None;
        self.pg_windows.reset();
        self.win_sel = 0;
    }

    fn zoom_to(&mut self, p: &'static str) {
        self.zoom = Some(p);
        self.focus = p;
    }
}

fn titled(name: &str, title: &str) -> String {
    format!("{}{title}", SUP[PANELS.iter().position(|p| *p == name).unwrap_or(0)])
}

/// (title, lines) for one panel; `h` = inner height when the panel is zoomed.
fn panel(st: &Store, ui: &mut Ui, name: &str, w: usize, h: Option<usize>) -> (String, Vec<String>) {
    let d = ui.d;
    match name {
        "today" => {
            let cap = match h {
                Some(h) => h.saturating_sub(TODAY_HEAD),
                None => ui.top + usize::from(ui.apps.len() > ui.top), // `top` apps plus the "more" line
            };
            let title = if st.group.get() == Group::Category {
                "by category"
            } else {
                "by app"
            };
            let sel = ui.sel;
            (
                title.into(),
                views::today_lines(st, d, w, Some(sel), ui.goal, cap, Some(&mut ui.pg_today)),
            )
        }
        "streaks" => (
            "streaks".into(),
            views::streak_lines(st, d, h.map_or(6, |h| h.saturating_sub(2).max(1)), w),
        ),
        "activity" => (
            format!("activity · {}", ui.graph),
            views::graph_lines(st, d, w, h.map_or(5, |h| h.saturating_sub(2).max(3)), ui.graph, None),
        ),
        "timeline" => {
            let lines = match h {
                Some(h) => views::timeline_lines(st, d, 12, w, Some(h), Some(&mut ui.pg_timeline)),
                None => views::timeline_lines(st, d, ui.top, w, None, None),
            };
            ("timeline".into(), lines)
        }
        "week" => (
            "week".into(),
            views::week_lines(st, d, h.map_or(7, |h| h.saturating_sub(4).max(7)), ui.top, w),
        ),
        "heatmap" => (
            "heatmap".into(),
            views::heat_lines(st, d, h.map_or(7, |h| h.saturating_sub(3).max(7)), w),
        ),
        _ => {
            let (s, e) = day_bounds(d);
            let sel = h.map(|_| ui.page_sel); // only the zoomed pages panel is selectable
            (
                "pages".into(),
                views::pages_lines(st, s, e, w, h.unwrap_or(12), "", &mut ui.pg_pages, sel),
            )
        }
    }
}

/// focus-track ‹ today · Wed 23 Sep › pages   ● tracking   1h19 of 6h00 goal   by app          14:02
fn header(st: &Store, ui: &Ui, w: usize) -> String {
    let th = theme();
    let (s, e) = day_bounds(ui.d);
    let total = sum(&totals(&st.by_group(s, e)));
    let strong = |x: &str| paint(&format!("1;{}", th.text), x);
    let status = if daemon_running() {
        paint(&th.app[1], &format!("{DOT} tracking"))
    } else {
        paint(&th.app[5], &format!("{DOT} not tracking"))
    };
    let crumb = match (&ui.detail, ui.zoom, ui.help) {
        (_, _, true) => " help".to_string(),
        (Some(app), _, _) => format!(" {app}"),
        (None, Some(z), _) => format!(" {z}"),
        _ => String::new(),
    };
    let left = format!(
        " {} {} {}{}   {status}   {} {}   {}",
        paint(&format!("1;{}", th.accent), "focus-track"),
        dim("‹"),
        strong(&day_label(ui.d)),
        dim(" ›") + &strong(&crumb),
        strong(&fmt(total)),
        dim(&format!("of {} goal", fmt(ui.goal))),
        dim(if st.group.get() == Group::Category {
            "by category"
        } else {
            "by app"
        }),
    );
    let clock = chrono::Local::now().format("%H:%M ").to_string();
    fit(&left, w.saturating_sub(clock.len())) + &dim(&clock)
}

/// The key bar: what the keys do in the current view.
fn key_bar(ui: &Ui, w: usize) -> String {
    let keys: &[(&str, &str)] = if ui.help {
        &[("esc", "close help"), ("q", "quit")]
    } else if ui.detail.is_some() {
        &[
            ("↑↓", "select window"),
            ("⏎", "open link"),
            ("esc", "back"),
            ("←→", "day"),
            ("?", "help"),
            ("q", "quit"),
        ]
    } else {
        match ui.zoom {
            Some("today") => &[
                ("↑↓", "select"),
                ("⏎", "details"),
                ("c", "apps/categories"),
                ("esc", "back"),
                ("←→", "day"),
                ("?", "help"),
            ],
            Some("pages") => &[
                ("↑↓", "select"),
                ("⏎", "open in browser"),
                ("esc", "back"),
                ("←→", "day"),
                ("?", "help"),
            ],
            Some("timeline") => &[("↑↓", "scroll"), ("esc", "back"), ("←→", "day"), ("?", "help")],
            Some("activity") => &[("g", "switches/apps"), ("esc", "back"), ("←→", "day"), ("?", "help")],
            Some(_) => &[("esc", "back"), ("←→", "day"), ("?", "help"), ("q", "quit")],
            None => &[
                ("↑↓", "select"),
                ("⏎", "details"),
                ("tab", "panel"),
                ("1-7", "zoom"),
                ("←→", "day"),
                ("c", "categories"),
                ("g", "graph"),
                ("?", "help"),
                ("q", "quit"),
            ],
        }
    };
    hints(keys, w)
}

fn help_lines() -> Vec<String> {
    let th = theme();
    let k = |key: &str, what: &str| format!("  {}  {}", paint(&format!("1;{}", th.accent), &format!("{key:<12}")), what);
    let sym = |s: String, what: &str| format!("  {}  {}", fit(&s, 12), dim(what));
    vec![
        dim("move"),
        k("↑ ↓  j k", "select the previous / next item in the list"),
        k("PgUp PgDn", "a page at a time"),
        k("Home End", "first / last item"),
        k("⏎  click", "open: an app's details, or a page in your browser"),
        k("esc  0", "back one level (quits from the overview)"),
        String::new(),
        dim("views"),
        k("tab  S-tab", "next / previous panel"),
        k("1 … 7", "zoom a panel full screen (or click its title)"),
        k("p", "pages you visited"),
        k("← →  h l  t", "previous / next day, today (or the mouse wheel over a panel)"),
        k("c", "group by app or by category (categories are set in the config)"),
        k("g", "activity graph: app switching or time per app"),
        k("q  ctrl-c", "quit"),
        String::new(),
        dim("how to read it"),
        sym(paint(&th.app[0], "■■■") + &paint(&th.border, "■■"), "how much time (longer = more)"),
        sym(
            paint(&th.app[0], &DOT.repeat(3)) + &paint(&th.border, &EMPTY.repeat(2)),
            "one dot = one minute (timeline) or one hour (heatmap) you were there",
        ),
        sym(paint(&th.heat[3], "⣀⣤⣶⣿"), "a trend through the day"),
        sym(
            paint(&th.app[4], "▲") + " " + &paint(&th.app[3], "▼") + " " + &dim("–"),
            "more / less / about your usual by this time of day",
        ),
        sym(paint(&th.app[1], &format!("{DOT} tracking")), "the recorder is running"),
    ]
}

/// The screen between the header and the key bar.
fn body(st: &Store, ui: &mut Ui, w: usize, h: usize) -> Vec<String> {
    if ui.help {
        return boxed("help", &help_lines(), w, Some(h), "", true);
    }
    if let Some(app) = ui.detail.clone() {
        let mut out = boxed(&app, &views::app_summary_lines(st, ui.d, &app), w, None, "", false);
        out.extend(boxed(
            "when",
            &views::graph_lines(st, ui.d, w - 4, 6, "apps", Some(&app)),
            w,
            None,
            "",
            false,
        ));
        let rest = h.saturating_sub(out.len());
        if rest >= 4 {
            let n = views::window_items(st, ui.d, &app).len();
            ui.win_sel = ui.win_sel.min(n.saturating_sub(1));
            let lines = views::windows_lines(st, ui.d, &app, w - 4, rest - 2, &mut ui.pg_windows, Some(ui.win_sel));
            out.extend(boxed("windows", &lines, w, Some(rest), "", true));
        }
        out.truncate(h);
        return out;
    }
    if let Some(z) = ui.zoom {
        if z == "pages" {
            let (s, e) = day_bounds(ui.d);
            let n = views::page_items(st, s, e, "").0.len();
            ui.page_sel = ui.page_sel.min(n.saturating_sub(1));
        }
        let (title, lines) = panel(st, ui, z, w - 4, Some(h - 2));
        ui.rects = vec![(z, (0, 0, w, h))];
        return boxed(&titled(z, &title), &lines, w, Some(h), "", true);
    }

    let mut out: Vec<String> = Vec::new();
    let add = |st: &Store, ui: &mut Ui, out: &mut Vec<String>, row: &[(&'static str, usize)]| {
        let built: Vec<(&'static str, usize, String, Vec<String>)> = row
            .iter()
            .map(|(name, pw)| {
                let (title, lines) = panel(st, ui, name, pw - 4, None);
                (*name, *pw, title, lines)
            })
            .collect();
        let bh = built.iter().map(|b| b.3.len()).max().unwrap_or(0) + 2;
        let mut x = 0;
        let mut boxes = Vec::new();
        for (name, pw, title, lines) in &built {
            ui.rects.push((name, (x, out.len(), *pw, bh)));
            boxes.push(boxed(&titled(name, title), lines, *pw, Some(bh), "", ui.focus == *name));
            x += pw;
        }
        out.extend(hstack(&boxes));
    };

    let lw = if w >= 110 { w * 11 / 20 } else { w };
    if lw < w {
        add(st, ui, &mut out, &[("today", lw), ("streaks", w - lw)]);
    } else {
        add(st, ui, &mut out, &[("today", w)]);
    }
    if h >= 28 {
        add(st, ui, &mut out, &[("activity", w)]);
    }
    let ww = if w >= 130 { w * 9 / 20 } else { w };
    let bottom_h = panel(st, ui, "week", ww - 4, None).1.len() + 2;
    let left = h.saturating_sub(out.len());
    // the week row only if the timeline still gets a useful height; otherwise the timeline takes it all
    let room = if left >= bottom_h + 6 { left - bottom_h } else { left };
    if room >= 6 {
        // the timeline gets what's left; older hours scroll off the top
        let (title, mut tl) = panel(st, ui, "timeline", w - 4, None);
        if tl.len() + 2 > room {
            let keep = room.saturating_sub(3).min(tl.len() - 1);
            let head = tl[0].clone();
            tl = std::iter::once(head).chain(tl[tl.len() - keep..].iter().cloned()).collect();
        }
        let th = room.min(tl.len() + 2);
        ui.rects.push(("timeline", (0, out.len(), w, th)));
        out.extend(boxed(&titled("timeline", &title), &tl, w, Some(th), "", ui.focus == "timeline"));
    }
    if h.saturating_sub(out.len()) >= bottom_h {
        if ww < w {
            add(st, ui, &mut out, &[("week", ww), ("heatmap", w - ww)]);
        } else {
            add(st, ui, &mut out, &[("week", w)]);
        }
    }
    out.truncate(h);
    out
}

pub fn frame(st: &Store, ui: &mut Ui, w: usize, h: usize) -> Vec<String> {
    ui.rects.clear();
    if w < MIN_W || h < MIN_H {
        let msg = format!("terminal too small ({w}x{h}): focus-track needs at least {MIN_W}x{MIN_H}");
        return vec![fit(&dim(&msg), w)];
    }
    let (s, e) = day_bounds(ui.d);
    ui.apps = totals(&st.by_group(s, e)).into_iter().map(|x| x.0).collect();
    match ui.sel_app.as_ref().and_then(|a| ui.apps.iter().position(|x| x == a)) {
        Some(i) => ui.sel = i,
        None => {
            ui.sel = ui.sel.min(ui.apps.len().saturating_sub(1));
            ui.sel_app = ui.apps.get(ui.sel).cloned();
        }
    }
    let mut out = vec![header(st, ui, w)];
    let mut b = body(st, ui, w, h - CHROME);
    for r in &mut ui.rects {
        r.1.1 += 1; // below the header bar
    }
    b.resize(h - CHROME, " ".repeat(w));
    out.extend(b);
    out.push(key_bar(ui, w));
    out
}

/// What the arrow keys move right now.
fn target(ui: &Ui) -> Option<&'static str> {
    if ui.help {
        return None;
    }
    if ui.detail.is_some() {
        return Some("windows");
    }
    match ui.zoom {
        Some("timeline") => Some("timeline"),
        Some("pages") => Some("pages"),
        None | Some("today") => Some("apps"),
        _ => None, // other zoomed panels have nothing to move
    }
}

fn moved(cur: usize, len: usize, page: usize, s: Step) -> usize {
    let (cur, last, page) = (cur as i64, len as i64 - 1, page.max(1) as i64);
    let i = match s {
        Step::Up => cur - 1,
        Step::Down => cur + 1,
        Step::PgUp => cur - page,
        Step::PgDn => cur + page,
        Step::Home => 0,
        Step::End => last,
    };
    i.clamp(0, last.max(0)) as usize
}

fn step(st: &Store, ui: &mut Ui, s: Step) {
    match target(ui) {
        Some("apps") => {
            if ui.apps.is_empty() {
                return;
            }
            ui.sel = moved(ui.sel, ui.apps.len(), ui.pg_today.n, s);
            ui.sel_app = Some(ui.apps[ui.sel].clone());
            if ui.zoom.is_none() {
                ui.focus = "today";
            }
        }
        Some("pages") => {
            let (a, b) = day_bounds(ui.d);
            let n = views::page_items(st, a, b, "").0.len();
            ui.page_sel = moved(ui.page_sel, n, ui.pg_pages.n, s);
        }
        Some("windows") => {
            let n = ui.detail.as_ref().map_or(0, |app| views::window_items(st, ui.d, app).len());
            ui.win_sel = moved(ui.win_sel, n, ui.pg_windows.n, s);
        }
        Some("timeline") => {
            let page = ui.pg_timeline.n.max(1) as i64;
            ui.pg_timeline.move_by(match s {
                Step::Up => -1,
                Step::Down => 1,
                Step::PgUp => -page,
                Step::PgDn => page,
                Step::Home => i64::MIN / 2,
                Step::End => i64::MAX / 2,
            });
        }
        _ => {}
    }
}

/// ⏎ on the current selection.
fn enter(st: &Store, ui: &mut Ui) {
    if let Some(app) = ui.detail.clone() {
        let items = views::window_items(st, ui.d, &app);
        if let Some(url) = items.get(ui.win_sel).and_then(|(k, _)| k.split_once('\t')).map(|x| x.1) {
            if safe_url(url) {
                (ui.opener)(url);
            }
        }
        return;
    }
    match ui.zoom {
        Some("pages") => {
            let (a, b) = day_bounds(ui.d);
            if let Some((url, _, _)) = views::page_items(st, a, b, "").0.get(ui.page_sel) {
                if safe_url(url) {
                    (ui.opener)(url);
                }
            }
        }
        Some("today") if !ui.apps.is_empty() => ui.open(ui.sel),
        None if ui.focus == "today" && !ui.apps.is_empty() => ui.open(ui.sel),
        None if ui.focus == "today" => {}
        None => ui.zoom = Some(ui.focus),
        _ => {}
    }
}

fn hit(ui: &Ui, x: usize, y: usize) -> Option<(&'static str, usize)> {
    ui.rects
        .iter()
        .find(|(_, (rx, ry, rw, rh))| (*rx..rx + rw).contains(&x) && (*ry..ry + rh).contains(&y))
        .map(|(n, r)| (*n, r.1))
}

/// Click on the `row`-th visible app line: select it, or open it if it's already selected.
fn pick(ui: &mut Ui, row: i64) {
    if row < 0 || row as usize >= ui.pg_today.n {
        return;
    }
    let i = ui.pg_today.scroll + row as usize;
    if i < ui.apps.len() {
        if ui.sel == i {
            ui.open(i);
        }
        ui.sel = i;
        ui.sel_app = Some(ui.apps[i].clone());
    }
}

fn click(ui: &mut Ui, x: usize, y: usize) {
    let Some((name, ry)) = hit(ui, x, y) else { return };
    let row = y as i64 - ry as i64 - 1 - TODAY_HEAD as i64;
    if ui.zoom.is_some() {
        if y == ry {
            ui.zoom = None; // title bar of the zoomed panel: back to the overview
        } else if name == "today" {
            pick(ui, row);
        }
        return;
    }
    ui.focus = name;
    if y == ry {
        ui.zoom = Some(name); // click a panel's title bar to zoom it
    } else if name == "today" {
        pick(ui, row);
    }
}

/// Apply one key or mouse event. Returns false to quit.
pub fn handle(st: &Store, ui: &mut Ui, input: Input) -> bool {
    if ui.help {
        // the help screen: Esc, ?, ⏎ or a click close it; q still quits; everything else is ignored
        match input {
            Input::Quit | Input::Char('q' | 'Q') => return false,
            Input::Esc | Input::Char('?' | '0') | Input::Enter | Input::Click(..) => ui.help = false,
            _ => {}
        }
        return true;
    }
    match input {
        Input::Quit | Input::Char('q' | 'Q') => return false,
        Input::Char('?') => ui.help = true,
        Input::Esc | Input::Char('0') => {
            if ui.detail.is_some() {
                ui.detail = None;
                ui.zoom = ui.back.take(); // back to where you came from, zoom included
            } else if ui.zoom.is_some() {
                ui.zoom = None;
            } else {
                return input != Input::Esc; // Esc at the top quits, like btop
            }
        }
        Input::Left | Input::Char('h') => ui.set_day(days_before(ui.d, 1)),
        Input::Right | Input::Char('l') => ui.set_day(days_before(ui.d, -1)),
        Input::Char('t') => ui.set_day(today()),
        Input::Up | Input::Char('k') => step(st, ui, Step::Up),
        Input::Down | Input::Char('j') => step(st, ui, Step::Down),
        Input::PgUp => step(st, ui, Step::PgUp),
        Input::PgDn => step(st, ui, Step::PgDn),
        Input::Home => step(st, ui, Step::Home),
        Input::End => step(st, ui, Step::End),
        Input::Tab | Input::BackTab if ui.zoom.is_none() && ui.detail.is_none() => {
            let shown: Vec<&'static str> = PANELS.iter().copied().filter(|p| ui.rects.iter().any(|r| r.0 == *p)).collect();
            if !shown.is_empty() {
                let i = shown.iter().position(|p| *p == ui.focus).map_or(-1, |i| i as i64);
                let n = shown.len() as i64;
                ui.focus = shown[((i + if input == Input::Tab { 1 } else { -1 }).rem_euclid(n)) as usize];
            }
        }
        Input::Enter => enter(st, ui),
        Input::Char(c @ '1'..='7') if ui.detail.is_none() => ui.zoom_to(PANELS[c as usize - '1' as usize]),
        Input::Char('p') if ui.detail.is_none() => ui.zoom_to("pages"),
        Input::Char('g') => ui.graph = if ui.graph == "switches" { "apps" } else { "switches" },
        Input::Char('c') if ui.detail.is_none() => {
            st.group.set(if st.group.get() == Group::Category {
                Group::App
            } else {
                Group::Category
            });
            ui.sel = 0;
            ui.sel_app = None;
            ui.pg_today.reset();
        }
        Input::Wheel(up, x, y) => {
            // the wheel moves a list when there is one under the pointer, else changes the day
            let over_today = hit(ui, x, y).is_some_and(|h| h.0 == "today");
            if matches!(target(ui), Some("windows" | "timeline" | "pages")) || over_today || ui.zoom == Some("today") {
                step(st, ui, if up { Step::Up } else { Step::Down });
            } else {
                ui.set_day(days_before(ui.d, if up { 1 } else { -1 }));
            }
        }
        Input::Click(x, y) if ui.detail.is_none() => click(ui, x, y),
        _ => {}
    }
    true
}

fn translate(ev: Event) -> Option<Input> {
    match ev {
        Event::Key(k) if k.kind != KeyEventKind::Release => Some(match k.code {
            KeyCode::Char('c') if k.modifiers.contains(KeyModifiers::CONTROL) => Input::Quit,
            KeyCode::Char(c) => Input::Char(c),
            KeyCode::Up => Input::Up,
            KeyCode::Down => Input::Down,
            KeyCode::PageUp => Input::PgUp,
            KeyCode::PageDown => Input::PgDn,
            KeyCode::Home => Input::Home,
            KeyCode::End => Input::End,
            KeyCode::Left => Input::Left,
            KeyCode::Right => Input::Right,
            KeyCode::Tab => Input::Tab,
            KeyCode::BackTab => Input::BackTab,
            KeyCode::Enter => Input::Enter,
            KeyCode::Esc => Input::Esc,
            _ => return None,
        }),
        Event::Mouse(m) => {
            let (x, y) = (m.column as usize, m.row as usize);
            match m.kind {
                MouseEventKind::ScrollUp => Some(Input::Wheel(true, x, y)),
                MouseEventKind::ScrollDown => Some(Input::Wheel(false, x, y)),
                MouseEventKind::Down(MouseButton::Left) => Some(Input::Click(x, y)),
                _ => None,
            }
        }
        _ => None,
    }
}

pub fn term_size() -> (usize, usize) {
    let env = |k: &str| std::env::var(k).ok().and_then(|v| v.parse::<usize>().ok()).filter(|v| *v > 0);
    let (w, h) = crossterm::terminal::size().map_or((80, 24), |(w, h)| (w as usize, h as usize));
    (env("COLUMNS").unwrap_or(w), env("LINES").unwrap_or(h))
}

/// Puts the terminal back however we leave (quit, error or panic).
struct Restore;

impl Drop for Restore {
    fn drop(&mut self) {
        let mut out = std::io::stdout();
        let _ = crossterm::execute!(
            out,
            crossterm::event::DisableMouseCapture,
            crossterm::cursor::Show,
            crossterm::terminal::LeaveAlternateScreen
        );
        let _ = crossterm::terminal::disable_raw_mode();
    }
}

pub fn run(st: &Store, ui: &mut Ui) -> anyhow::Result<()> {
    use std::io::IsTerminal;
    if !(std::io::stdin().is_terminal() && std::io::stdout().is_terminal()) {
        let (w, h) = term_size();
        println!("{}", frame(st, ui, w, h).join("\n"));
        return Ok(());
    }
    let mut out = std::io::stdout();
    crossterm::terminal::enable_raw_mode()?;
    let _restore = Restore;
    crossterm::execute!(
        out,
        crossterm::terminal::EnterAlternateScreen,
        crossterm::cursor::Hide,
        crossterm::event::EnableMouseCapture
    )?;
    loop {
        let (w, h) = crossterm::terminal::size().map_or((80, 24), |(w, h)| (w as usize, h as usize));
        let lines = frame(st, ui, w, h);
        write!(out, "\x1b[H{}\x1b[K\x1b[J", lines.join("\x1b[K\r\n"))?;
        out.flush()?;
        // redraw every 2 s (live data, clock), or as soon as something relevant happens
        if !event::poll(Duration::from_secs(2))? {
            continue;
        }
        loop {
            let ev = event::read()?;
            let resized = matches!(ev, Event::Resize(..));
            match translate(ev) {
                Some(input) if !handle(st, ui, input) => return Ok(()),
                Some(_) => break,
                None if resized => break,
                None => {}
            }
            // mouse motion and other noise: keep reading without redrawing
            if !event::poll(Duration::from_secs(2))? {
                break;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::render::{set_tty, strip_ansi, vlen};
    use crate::store::test_store;
    use crate::util::midnight;
    use std::collections::HashSet;
    use std::sync::Mutex;

    /// 40 apps (app00 biggest); app03 visits 37 pages, app01 has 39 window titles.
    fn stress_store(d: NaiveDate) -> Store {
        let st = test_store("");
        let n = 40;
        let mut t = midnight(d) + 7.0 * 3600.0;
        let (mut ff, mut gh) = (0, 0);
        for r in 0..n {
            for i in 0..n {
                if n - i > r {
                    let app = format!("app{i:02}");
                    let (title, url) = match i {
                        3 => {
                            ff += 1;
                            (
                                format!("Page {:02} title", ff - 1),
                                format!("https://site{}.example.com/path", ff - 1),
                            )
                        }
                        1 => {
                            gh += 1;
                            (format!("vim file{}.py", gh - 1), String::new())
                        }
                        _ => (String::new(), String::new()),
                    };
                    st.con
                        .execute(
                            "INSERT INTO focus (ts, app, title, url, mode) VALUES (?1, ?2, ?3, ?4, '')",
                            rusqlite::params![t, app, title, url],
                        )
                        .unwrap();
                    t += 60.0;
                }
            }
        }
        st.con
            .execute("INSERT INTO focus (ts, app, title, url, mode) VALUES (?1, '', '', '', '')", [t])
            .unwrap();
        st
    }

    fn yesterday() -> NaiveDate {
        days_before(today(), 1)
    }

    fn new_ui() -> Ui {
        Ui::new(yesterday(), 8, 6.0 * 3600.0)
    }

    /// Every frame fills the terminal exactly, with the header on top and the key bar at the bottom.
    fn render(st: &Store, ui: &mut Ui, w: usize, h: usize) -> Vec<String> {
        let f = frame(st, ui, w, h);
        assert_eq!(f.len(), h, "frame has {} lines, terminal {h}", f.len());
        for (i, l) in f.iter().enumerate() {
            assert_eq!(vlen(l), w, "line {i} is {} wide (want {w}): {}", vlen(l), strip_ansi(l));
        }
        assert!(strip_ansi(&f[0]).contains("focus-track"), "header on top");
        let bar = strip_ansi(&f[h - 1]);
        assert!(
            bar.contains("? help") || bar.contains("close help"),
            "key bar at the bottom always offers help: {bar}"
        );
        f
    }

    /// Text of reversed (selected) rows.
    fn selected(f: &[String]) -> Vec<String> {
        f.iter()
            .filter_map(|l| l.find("\x1b[7;").map(|i| &l[i..]))
            .map(|rest| strip_ansi(rest.split("\x1b[0m").next().unwrap_or("")).trim().to_string())
            .collect()
    }

    fn text(f: &[String]) -> String {
        f.iter().map(|l| strip_ansi(l)).collect::<Vec<_>>().join("\n")
    }

    const SIZES: [(usize, usize); 7] = [(160, 54), (120, 42), (100, 32), (80, 26), (MIN_W, MIN_H), (200, 62), (70, 22)];

    #[test]
    fn every_app_is_reachable_in_every_size() {
        set_tty(true);
        let st = stress_store(yesterday());
        for (w, h) in SIZES {
            for zoomed in [false, true] {
                let mut ui = new_ui();
                render(&st, &mut ui, w, h);
                assert_eq!(ui.apps.len(), 40);
                if zoomed {
                    handle(&st, &mut ui, Input::Char('1'));
                }
                let mut seen = HashSet::new();
                for _ in 0..43 {
                    let f = render(&st, &mut ui, w, h);
                    let pg = &ui.pg_today;
                    assert!(pg.scroll <= ui.sel && ui.sel < pg.scroll + pg.n, "{w}x{h}: selection off screen");
                    let sel = selected(&f);
                    assert_eq!(sel.len(), 1, "{w}x{h}: exactly one selected row");
                    assert!(
                        sel[0].starts_with(&ui.apps[ui.sel]),
                        "{w}x{h}: the whole selected row is reversed: {sel:?}"
                    );
                    seen.insert(ui.sel);
                    handle(&st, &mut ui, Input::Down);
                }
                assert_eq!(seen.len(), 40, "{w}x{h} zoomed={zoomed}");
                for _ in 0..45 {
                    handle(&st, &mut ui, Input::Up);
                }
                assert_eq!(ui.sel, 0);
            }
        }
    }

    #[test]
    fn keys_detail_and_back() {
        set_tty(true);
        let st = stress_store(yesterday());
        let mut ui = new_ui();
        assert!(text(&render(&st, &mut ui, 120, 42)).contains("↓ 32 more"));
        let n = ui.pg_today.n;
        assert!(handle(&st, &mut ui, Input::PgDn) && ui.zoom.is_none());
        assert_eq!(ui.sel, n);
        handle(&st, &mut ui, Input::End);
        assert_eq!(ui.sel, 39);
        handle(&st, &mut ui, Input::Home);
        assert_eq!(ui.sel, 0);

        handle(&st, &mut ui, Input::Char('1'));
        handle(&st, &mut ui, Input::End);
        render(&st, &mut ui, 120, 42);
        handle(&st, &mut ui, Input::Enter);
        assert_eq!(ui.detail.as_deref(), Some("app39"));
        let f = render(&st, &mut ui, 120, 42);
        assert!(strip_ansi(&f[0]).contains("› app39"), "breadcrumb in the header");
        handle(&st, &mut ui, Input::Esc);
        assert_eq!((ui.detail.as_deref(), ui.zoom, ui.sel), (None, Some("today"), 39)); // back to the zoom
        handle(&st, &mut ui, Input::Esc);
        assert_eq!(ui.zoom, None);
        assert!(!handle(&st, &mut ui, Input::Esc)); // Esc at the top quits

        // the windows list of app01: every title reachable, selection moves
        let mut ui = new_ui();
        ui.sel_app = Some("app01".into());
        render(&st, &mut ui, 100, 32);
        handle(&st, &mut ui, Input::Enter);
        let mut titles = HashSet::new();
        for _ in 0..60 {
            let f = render(&st, &mut ui, 100, 32);
            for l in &f {
                let l = strip_ansi(l);
                if let Some(i) = l.find("vim file") {
                    titles.insert(l[i..].split_whitespace().nth(1).unwrap_or("").to_string());
                }
            }
            assert_eq!(selected(&f).len(), 1, "one selected window");
            handle(&st, &mut ui, Input::Down);
        }
        assert_eq!(titles.len(), 39); // app01 has one window per round it appears in
        assert_eq!(ui.win_sel, 38);

        // pages: all 37 reachable, with a selected row
        let mut ui = new_ui();
        render(&st, &mut ui, 120, 32);
        handle(&st, &mut ui, Input::Char('p'));
        let mut pages = HashSet::new();
        for _ in 0..80 {
            let f = render(&st, &mut ui, 120, 32);
            for l in &f {
                let l = strip_ansi(l);
                if let Some(i) = l.find("Page ") {
                    pages.insert(l[i..i + 7].to_string());
                }
            }
            assert_eq!(selected(&f).len(), 1, "one selected page");
            handle(&st, &mut ui, Input::Down);
        }
        assert_eq!(pages.len(), 37);
        assert_eq!(ui.page_sel, 36);
        assert_eq!(ui.sel, 0, "arrows in pages don't move the hidden app selection");
    }

    static OPENED: Mutex<Vec<String>> = Mutex::new(Vec::new());

    fn fake_open(url: &str) -> bool {
        OPENED.lock().unwrap().push(url.to_string());
        true
    }

    #[test]
    fn enter_opens_links_safely() {
        set_tty(true);
        let st = stress_store(yesterday());
        st.con
            .execute_batch(&format!(
                "INSERT INTO focus (ts, app, title, url) VALUES ({0}, 'app03', 'Evil', 'javascript:alert(1)'), ({1}, '', '', '')",
                midnight(yesterday()) + 22.0 * 3600.0,
                midnight(yesterday()) + 23.0 * 3600.0
            ))
            .unwrap();
        let mut ui = new_ui();
        ui.opener = fake_open;
        render(&st, &mut ui, 120, 42);
        handle(&st, &mut ui, Input::Char('p'));
        render(&st, &mut ui, 120, 42);
        let (a, b) = day_bounds(yesterday());
        let items = views::page_items(&st, a, b, "").0;
        ui.page_sel = items.iter().position(|p| p.0.starts_with("https")).unwrap();
        handle(&st, &mut ui, Input::Enter);
        let first = OPENED.lock().unwrap().last().cloned();
        assert!(first.is_some_and(|u| u.starts_with("https://site")), "⏎ opens the selected page");
        // a javascript: "page" must never reach the opener
        let evil = items.iter().position(|p| p.0.starts_with("javascript")).unwrap();
        ui.page_sel = evil;
        let before = OPENED.lock().unwrap().len();
        handle(&st, &mut ui, Input::Enter);
        assert_eq!(OPENED.lock().unwrap().len(), before, "non-web links are never opened");

        // a window without a URL: ⏎ does nothing
        let mut ui = new_ui();
        ui.opener = fake_open;
        ui.sel_app = Some("app01".into());
        render(&st, &mut ui, 120, 42);
        handle(&st, &mut ui, Input::Enter);
        let before = OPENED.lock().unwrap().len();
        handle(&st, &mut ui, Input::Enter);
        assert_eq!(OPENED.lock().unwrap().len(), before);

        assert!(safe_url("https://example.com/a?b=c"));
        assert!(!safe_url("file:///etc/passwd"));
        assert!(!safe_url("https://x.com/\x1b]8;;"));
        assert!(!safe_url("https://x.com/ --flag"));
        assert!(!safe_url("https://"));
    }

    #[test]
    fn help_screen() {
        set_tty(true);
        let st = stress_store(yesterday());
        let mut ui = new_ui();
        render(&st, &mut ui, 100, 32);
        handle(&st, &mut ui, Input::Char('?'));
        let f = render(&st, &mut ui, 100, 32);
        let t = text(&f);
        assert!(
            t.contains("how to read it") && t.contains("one dot = one minute"),
            "help explains the symbols"
        );
        assert!(strip_ansi(&f[31]).contains("close help"));
        handle(&st, &mut ui, Input::Down);
        assert_eq!(ui.sel, 0, "keys don't leak through the help screen");
        assert!(handle(&st, &mut ui, Input::Esc), "Esc closes help, it doesn't quit");
        assert!(!ui.help);
        handle(&st, &mut ui, Input::Char('?'));
        assert!(!handle(&st, &mut ui, Input::Char('q')), "q still quits from help");
    }

    #[test]
    fn one_visual_language() {
        set_tty(true);
        let st = stress_store(yesterday());
        for (w, h) in SIZES {
            for key in [
                None,
                Some('1'),
                Some('2'),
                Some('3'),
                Some('4'),
                Some('5'),
                Some('6'),
                Some('7'),
                Some('?'),
            ] {
                let mut ui = new_ui();
                render(&st, &mut ui, w, h);
                if let Some(k) = key {
                    handle(&st, &mut ui, Input::Char(k));
                }
                let t = text(&render(&st, &mut ui, w, h));
                for glyph in ['█', '▓', '▒', '░', '▌', '▏'] {
                    assert!(
                        !t.contains(glyph),
                        "{w}x{h} key={key:?}: old block glyph {glyph} (dots, ■ meters and braille only)"
                    );
                }
                if key.is_none() {
                    let f = render(&st, &mut ui, w, h);
                    let blank = f.iter().rev().skip(1).take_while(|l| strip_ansi(l).trim().is_empty()).count();
                    assert!(blank < 6, "{w}x{h}: {blank} empty lines at the bottom of the overview");
                }
                if key == Some('4') || key == Some('6') {
                    assert!(t.contains(DOT), "{w}x{h}: time grids are made of dots");
                }
            }
        }
    }

    #[test]
    fn key_bar_matches_the_view() {
        set_tty(true);
        let st = stress_store(yesterday());
        let bar = |ui: &mut Ui| strip_ansi(render(&st, ui, 120, 42).last().unwrap());
        let mut ui = new_ui();
        assert!(bar(&mut ui).contains("1-7 zoom"));
        handle(&st, &mut ui, Input::Char('p'));
        assert!(bar(&mut ui).contains("open in browser"));
        handle(&st, &mut ui, Input::Esc);
        handle(&st, &mut ui, Input::Char('4'));
        assert!(bar(&mut ui).contains("scroll"));
        handle(&st, &mut ui, Input::Esc);
        handle(&st, &mut ui, Input::Down); // arrows select an app, which puts the focus back on the app list
        handle(&st, &mut ui, Input::Enter);
        assert!(bar(&mut ui).contains("select window"));
    }

    #[test]
    fn key_bar_always_offers_help_and_quit() {
        set_tty(true);
        for w in [MIN_W, 70, 90, 200] {
            let bar = strip_ansi(&key_bar(&new_ui(), w));
            assert!(bar.contains("? help") && bar.contains("q quit"), "{w}: {bar}");
            assert_eq!(unicode_width::UnicodeWidthStr::width(bar.as_str()), w);
        }
        assert!(
            strip_ansi(&key_bar(&new_ui(), 200)).contains("1-7 zoom"),
            "wide terminals show everything"
        );
    }

    #[test]
    fn tab_cycles_every_panel() {
        set_tty(true);
        let st = stress_store(yesterday());
        let mut ui = new_ui();
        render(&st, &mut ui, 160, 54);
        let mut order = vec![ui.focus];
        for _ in 0..6 {
            handle(&st, &mut ui, Input::Tab);
            render(&st, &mut ui, 160, 54);
            order.push(ui.focus);
        }
        assert_eq!(order, ["today", "streaks", "activity", "timeline", "week", "heatmap", "today"]);
        handle(&st, &mut ui, Input::BackTab);
        assert_eq!(ui.focus, "heatmap");
        handle(&st, &mut ui, Input::Enter);
        assert_eq!(ui.zoom, Some("heatmap"), "⏎ on a panel zooms it");
    }

    #[test]
    fn mouse_and_categories() {
        set_tty(true);
        let d = yesterday();
        let st = stress_store(d);
        let mut ui = new_ui();
        render(&st, &mut ui, 120, 42);
        let (_, (_, ry, _, _)) = *ui.rects.iter().find(|r| r.0 == "today").unwrap();
        let row = |k: usize| ry + 1 + TODAY_HEAD + k;
        handle(&st, &mut ui, Input::Click(5, row(3)));
        assert_eq!(ui.sel, 3);
        render(&st, &mut ui, 120, 42);
        handle(&st, &mut ui, Input::Click(5, row(3)));
        assert_eq!(ui.detail.as_deref(), Some("app03"));
        handle(&st, &mut ui, Input::Esc);
        render(&st, &mut ui, 120, 42);
        handle(&st, &mut ui, Input::Wheel(false, 5, row(0)));
        assert_eq!((ui.sel, ui.d), (4, d), "wheel over the app list moves the selection");
        let (_, (sx, sy, _, _)) = *ui.rects.iter().find(|r| r.0 == "streaks").unwrap();
        handle(&st, &mut ui, Input::Wheel(true, sx + 5, sy + 3));
        assert_eq!(ui.d, days_before(d, 1), "wheel elsewhere changes the day");
        ui.set_day(d);
        render(&st, &mut ui, 120, 42);
        let (_, (hx, hy, _, _)) = *ui.rects.iter().find(|r| r.0 == "timeline").unwrap();
        handle(&st, &mut ui, Input::Click(hx + 3, hy));
        assert_eq!(ui.zoom, Some("timeline"), "click a title to zoom");
        render(&st, &mut ui, 120, 42);
        handle(&st, &mut ui, Input::Click(3, 1));
        assert_eq!(ui.zoom, None, "click the zoomed title to go back");
        assert!(handle(&st, &mut ui, Input::Click(0, 0)), "clicks on the header are harmless");

        // every screen in every size, with category grouping on
        handle(&st, &mut ui, Input::Char('c'));
        assert_eq!(st.group.get(), Group::Category);
        for (w, h) in SIZES {
            for z in [None, Some('1'), Some('2'), Some('3'), Some('4'), Some('5'), Some('6'), Some('7')] {
                let mut ui = new_ui();
                render(&st, &mut ui, w, h);
                if let Some(z) = z {
                    handle(&st, &mut ui, Input::Char(z));
                }
                for _ in 0..3 {
                    handle(&st, &mut ui, Input::Down);
                    handle(&st, &mut ui, Input::PgDn);
                    render(&st, &mut ui, w, h);
                }
            }
        }
    }

    #[test]
    fn empty_day_and_tiny_terminal() {
        set_tty(true);
        let st = test_store("");
        let mut ui = new_ui();
        let t = text(&render(&st, &mut ui, 100, 32));
        assert!(t.contains("nothing tracked"), "empty days say so and suggest what to do");
        for input in [
            Input::Enter,
            Input::Down,
            Input::Char('p'),
            Input::Enter,
            Input::Esc,
            Input::Char('4'),
            Input::PgDn,
            Input::Esc,
        ] {
            handle(&st, &mut ui, input);
            render(&st, &mut ui, 100, 32);
        }
        let small = frame(&st, &mut ui, 50, 10);
        assert!(strip_ansi(&small[0]).contains("too small"));
    }
}
