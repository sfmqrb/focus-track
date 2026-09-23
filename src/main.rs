//! focus-track: see where your time goes on Hyprland.

mod backup;
mod config;
mod dashboard;
mod doctor;
mod render;
mod store;
mod track;
mod util;
mod views;

use chrono::NaiveDate;
use clap::{CommandFactory, Parser, ValueEnum};
use std::io::IsTerminal;
use store::{Group, Store, merged, totals};
use util::{day_bounds, days_before, now, today};

#[derive(Clone, Copy, PartialEq, Eq, ValueEnum)]
enum Cmd {
    /// Bar chart of today's apps (the default)
    Today,
    /// Bar chart of yesterday's apps
    Yesterday,
    /// Live full-screen dashboard (btop style)
    Dashboard,
    /// Stacked chart per day for the last --days days
    Week,
    /// Minute-by-minute map of the day
    Timeline,
    /// Browser pages with their URLs (--search to filter; private windows are never stored)
    Pages,
    /// Braille graph of app switching through the day (--apps: stacked by app)
    Graph,
    /// Longest unbroken stretches and app switches per hour
    Streaks,
    /// Days x hours grid of focused time (default 28 days)
    Heatmap,
    /// Every window class seen, what it's shown as and its category
    Apps,
    /// Check that tracking works: daemon, gaps, idle/lock, audio, browsers, config
    Doctor,
    /// Print the config file path (creates a commented starter file if missing)
    Config,
    /// Back up the database now (the recorder also does it once a day)
    Backup,
    /// Send the end-of-day notification now
    Summary,
    /// CSV of every span (all data, or a --days/--date range)
    Export,
    /// One JSON line for a Waybar-style text module
    Bar,
    /// JSON for the Omarchy bar ring widget
    Widget,
    /// Record which window has focus (run this in the background)
    Daemon,
    /// Print shell completions (bash, zsh, fish, elvish, powershell)
    Completions,
}

#[derive(Parser)]
#[command(
    name = "focus-track",
    version,
    about = "See where your time goes on Hyprland: tracks the focused app, with a btop-style dashboard."
)]
struct Cli {
    #[arg(value_enum, default_value_t = Cmd::Today, hide_possible_values = false)]
    command: Cmd,
    /// Shell for `completions`
    #[arg(value_enum)]
    shell: Option<clap_complete::Shell>,
    /// Show this day instead of today (multi-day views: the last day). YYYY-MM-DD
    #[arg(short, long, value_parser = parse_date)]
    date: Option<NaiveDate>,
    /// How many apps get their own row/color before "other"
    #[arg(short = 'n', long, default_value_t = 8, value_parser = clap::value_parser!(u16).range(1..))]
    top: u16,
    /// Number of days for week/heatmap/export/pages
    #[arg(short = 'D', long, value_parser = clap::value_parser!(u16).range(1..))]
    days: Option<u16>,
    /// Daily focus goal in hours
    #[arg(short, long, default_value_t = 6.0, value_parser = parse_goal)]
    goal: f64,
    /// pages: only titles/URLs containing this text
    #[arg(short, long, default_value = "")]
    search: String,
    /// Group every view by category instead of app
    #[arg(short, long)]
    categories: bool,
    /// graph: stack by app instead of showing app switching
    #[arg(long)]
    apps: bool,
    /// Raw seconds per app as JSON
    #[arg(long)]
    json: bool,
}

fn parse_date(s: &str) -> Result<NaiveDate, String> {
    NaiveDate::parse_from_str(s, "%Y-%m-%d").map_err(|e| format!("{e} (use YYYY-MM-DD)"))
}

fn parse_goal(s: &str) -> Result<f64, String> {
    let g: f64 = s.parse().map_err(|_| "not a number".to_string())?;
    if g > 0.0 && g <= 24.0 {
        Ok(g)
    } else {
        Err("must be between 0 and 24 hours".into())
    }
}

fn single(title: &str, lines: impl FnOnce(usize) -> Vec<String>) {
    let w = dashboard::term_size().0.max(30);
    println!("{}", render::boxed(title, &lines(w - 4), w, None, "", false).join("\n"));
}

fn csv_field(s: &str) -> String {
    if s.contains([',', '"', '\n']) {
        format!("\"{}\"", s.replace('"', "\"\""))
    } else {
        s.to_string()
    }
}

fn main() -> anyhow::Result<()> {
    let a = Cli::parse();
    render::set_tty(std::io::stdout().is_terminal() && std::env::var_os("NO_COLOR").is_none());
    let top = usize::from(a.top);
    let days = a.days.map(i64::from);
    let d = a.date.unwrap_or_else(|| {
        if a.command == Cmd::Yesterday {
            days_before(today(), 1)
        } else {
            today()
        }
    });

    match a.command {
        Cmd::Daemon => return track::daemon(),
        Cmd::Completions => {
            let shell = a
                .shell
                .or_else(clap_complete::Shell::from_env)
                .unwrap_or(clap_complete::Shell::Bash);
            clap_complete::generate(shell, &mut Cli::command(), "focus-track", &mut std::io::stdout());
            return Ok(());
        }
        Cmd::Config => {
            let path = config::path();
            if !path.exists() {
                if let Some(dir) = path.parent() {
                    std::fs::create_dir_all(dir)?;
                }
                std::fs::write(&path, config::STARTER)?;
            }
            println!("{}", path.display());
            return Ok(());
        }
        _ => {}
    }

    let st = Store::open()?;
    if a.categories {
        st.group.set(Group::Category);
    }
    let goal = a.goal * 3600.0;
    match a.command {
        Cmd::Widget => println!("{}", views::widget_json(&st)),
        Cmd::Summary => track::send_summary(&st),
        Cmd::Backup => {
            let dir = backup::dir();
            let file = backup::make(&st.con, &dir, today())?;
            let all = backup::list(&dir);
            let size: u64 = all.iter().filter_map(|(_, p)| std::fs::metadata(p).ok()).map(|m| m.len()).sum();
            println!("backed up to {} (integrity checked)", file.display());
            println!(
                "{} backups, {:.1} MB, oldest {}  (kept: daily for 2 weeks, weekly for 8 weeks, monthly forever)",
                all.len(),
                size as f64 / 1e6,
                all.first().map_or_else(String::new, |x| x.0.to_string())
            );
        }
        Cmd::Bar => {
            let (s, e) = day_bounds(today());
            let t = totals(&st.by_group(s, e));
            let tip: Vec<String> = t.iter().take(top).map(|(a, s)| format!("{:>6}  {a}", util::fmt(*s))).collect();
            let tip = if tip.is_empty() {
                "nothing tracked yet".to_string()
            } else {
                tip.join("\n")
            };
            let text = format!("󱎫 {}", util::fmt(store::sum(&t)));
            println!(
                "{}",
                serde_json::json!({"text": text, "tooltip": format!("{tip}\n\nclick for dashboard"), "class": "focus"})
            );
        }
        Cmd::Export => {
            let first: Option<f64> = st.con.query_row("SELECT min(ts) FROM focus", [], |r| r.get(0)).ok().flatten();
            let start = days.map_or(first.unwrap_or_else(now), |n| day_bounds(days_before(d, n - 1)).0);
            println!("start,end,seconds,class,name,category,mode");
            let key = |r: &store::Row| {
                if r.app.is_empty() {
                    String::new()
                } else {
                    [
                        r.app.to_string(),
                        st.cfg.name_of(r.app),
                        st.cfg.category_of(r.app, r.url),
                        r.mode.to_string(),
                    ]
                    .join("\t")
                }
            };
            for (s, e, k) in merged(&st.spans(start, day_bounds(d).1, &key)) {
                let fields: Vec<String> = k.split('\t').map(csv_field).collect();
                let t = |ts: f64| util::local(ts).format("%Y-%m-%dT%H:%M:%S").to_string();
                println!("{},{},{},{}", t(s), t(e), (e - s).round() as i64, fields.join(","));
            }
        }
        Cmd::Pages => {
            let start = day_bounds(days_before(d, days.unwrap_or(1) - 1)).0;
            let end = day_bounds(d).1;
            if a.json {
                let t = totals(&st.spans(start, end, &views::page_key));
                let v: Vec<serde_json::Value> = t
                    .iter()
                    .filter(|(k, _)| !k.starts_with('\t'))
                    .filter_map(|(k, s)| {
                        k.split_once('\t')
                            .map(|(u, ti)| serde_json::json!({"url": u, "title": ti, "seconds": s.round() as i64}))
                    })
                    .collect();
                println!("{}", serde_json::to_string_pretty(&v)?);
            } else {
                let label = if days.unwrap_or(1) == 1 {
                    views::day_label(d)
                } else {
                    format!("last {} days", days.unwrap_or(1))
                };
                single(&format!("pages · {label}"), |w| {
                    views::pages_lines(&st, start, end, w, usize::MAX, &a.search, &mut views::Pager::default(), None)
                });
            }
        }
        _ if a.json => {
            let n = if matches!(a.command, Cmd::Week | Cmd::Heatmap) {
                days.unwrap_or(7)
            } else {
                1
            };
            let mut out = serde_json::Map::new();
            for i in 0..n {
                let x = days_before(d, i);
                let (s, e) = day_bounds(x);
                let t: serde_json::Map<String, serde_json::Value> = totals(&st.by_group(s, e))
                    .into_iter()
                    .map(|(k, v)| (k, serde_json::json!(v.round() as i64)))
                    .collect();
                out.insert(x.to_string(), serde_json::Value::Object(t));
            }
            println!("{}", serde_json::to_string_pretty(&out)?);
        }
        Cmd::Dashboard => dashboard::run(&st, &mut dashboard::Ui::new(d, top, goal))?,
        Cmd::Week => single("week", |w| views::week_lines(&st, d, days.unwrap_or(7) as usize, top, w)),
        Cmd::Heatmap => single("heatmap", |w| views::heat_lines(&st, d, days.unwrap_or(28) as usize, w)),
        Cmd::Graph => single(&format!("activity · {}", views::day_label(d)), |w| {
            views::graph_lines(&st, d, w, 8, if a.apps { "apps" } else { "switches" }, None)
        }),
        Cmd::Timeline => single(&format!("timeline · {}", views::day_label(d)), |w| {
            views::timeline_lines(&st, d, top, w, None, None)
        }),
        Cmd::Streaks => single(&format!("streaks · {}", views::day_label(d)), |w| {
            views::streak_lines(&st, d, top, w)
        }),
        Cmd::Apps => single("apps", |_| views::apps_lines(&st, days.unwrap_or(7))),
        Cmd::Doctor => {
            let (lines, broken) = doctor::doctor_lines(&st);
            single("doctor", |_| lines);
            if broken {
                std::process::exit(1);
            }
        }
        _ => single(&views::day_label(d), |w| views::today_lines(&st, d, w, None, goal, top, None)),
    }
    Ok(())
}
