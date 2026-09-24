//! Runs the real `focus-track` binary against a throwaway database, config and backup folder.

use rusqlite::Connection;
use std::path::PathBuf;
use std::process::{Command, Output};

struct Sandbox {
    dir: PathBuf,
}

impl Sandbox {
    fn new(name: &str) -> Sandbox {
        let dir = std::env::temp_dir().join(format!("focus-track-cli-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        Sandbox { dir }
    }

    fn db(&self) -> PathBuf {
        self.dir.join("focus.db")
    }

    /// Yesterday 09:00-10:00 code, 10:00-10:30 firefox on a page, then a browser row with no URL (private).
    fn with_day(self) -> Sandbox {
        let con = Connection::open(self.db()).unwrap();
        con.execute_batch("CREATE TABLE focus (ts REAL, app TEXT, title TEXT DEFAULT '', url TEXT, mode TEXT DEFAULT '')")
            .unwrap();
        let start = chrono::Local::now().date_naive().pred_opt().unwrap().and_hms_opt(9, 0, 0).unwrap();
        let t0 = start.and_local_timezone(chrono::Local).earliest().unwrap().timestamp() as f64;
        let add = |from: f64, to: f64, app: &str, title: &str, url: &str| {
            let mut t = t0 + from * 60.0;
            while t < t0 + to * 60.0 {
                con.execute(
                    "INSERT INTO focus VALUES (?1, ?2, ?3, ?4, '')",
                    rusqlite::params![t, app, title, url],
                )
                .unwrap();
                t += 240.0;
            }
            con.execute("INSERT INTO focus VALUES (?1, '', '', '', '')", [t0 + to * 60.0])
                .unwrap();
        };
        add(0.0, 60.0, "code", "main.rs", "");
        add(60.0, 90.0, "firefox", "Docs, and more", "https://docs.example/a,b");
        add(120.0, 135.0, "firefox", "", "");
        self
    }

    fn run(&self, args: &[&str]) -> Output {
        Command::new(env!("CARGO_BIN_EXE_focus-track"))
            .args(args)
            .env("FOCUS_TRACK_DB", self.db())
            .env("FOCUS_TRACK_CONFIG", self.dir.join("config.toml"))
            .env("FOCUS_TRACK_BACKUP_DIR", self.dir.join("backups"))
            .env("FOCUS_TRACK_THEME", self.dir.join("no-theme"))
            .env("HOME", &self.dir)
            .env("XDG_CONFIG_HOME", self.dir.join("config"))
            .env("XDG_CACHE_HOME", self.dir.join("cache"))
            .env("COLUMNS", "100")
            .env_remove("NO_COLOR")
            .output()
            .unwrap()
    }

    fn ok(&self, args: &[&str]) -> String {
        let out = self.run(args);
        assert!(out.status.success(), "{args:?} failed: {}", String::from_utf8_lossy(&out.stderr));
        String::from_utf8(out.stdout).unwrap()
    }
}

impl Drop for Sandbox {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.dir);
    }
}

fn yesterday() -> String {
    chrono::Local::now().date_naive().pred_opt().unwrap().to_string()
}

#[test]
fn help_and_version() {
    let s = Sandbox::new("help");
    assert!(s.ok(&["--version"]).starts_with("focus-track "));
    let help = s.ok(&["--help"]);
    for cmd in ["dashboard", "pages", "doctor", "backup", "export", "completions"] {
        assert!(help.contains(cmd), "--help lists {cmd}");
    }
}

#[test]
fn output_to_a_pipe_has_no_colors_or_escapes() {
    let s = Sandbox::new("pipe").with_day();
    for args in [
        vec!["-d", &yesterday()],
        vec!["week"],
        vec!["pages", "-d", &yesterday()],
        vec!["apps"],
    ] {
        let out = s.ok(&args.iter().map(|a| a.as_ref()).collect::<Vec<&str>>());
        assert!(!out.contains('\x1b'), "{args:?} printed escape codes to a pipe");
    }
    let today = s.ok(&["-d", &yesterday()]);
    assert!(
        today.contains("1h45 focused") && today.contains("code") && today.contains("firefox"),
        "{today}"
    );
}

#[test]
fn empty_database() {
    let s = Sandbox::new("empty");
    assert!(s.ok(&[]).contains("nothing tracked"));
    assert!(
        s.ok(&["dashboard"]).contains("focus-track"),
        "the dashboard prints one frame when not in a terminal"
    );
}

#[test]
fn json_and_export() {
    let s = Sandbox::new("json").with_day();
    let v: serde_json::Value = serde_json::from_str(&s.ok(&["--json", "-d", &yesterday()])).unwrap();
    assert_eq!(v[yesterday()]["code"], 3600);
    assert_eq!(v[yesterday()]["firefox"], 2700);

    let pages: serde_json::Value = serde_json::from_str(&s.ok(&["pages", "--json", "-d", &yesterday()])).unwrap();
    let pages = pages.as_array().unwrap();
    assert_eq!(pages.len(), 1, "private time is not a page: {pages:?}");
    assert_eq!(pages[0]["url"], "https://docs.example/a,b");
    assert_eq!(pages[0]["seconds"], 1800);

    let csv = s.ok(&["export", "-D", "2", "-d", &yesterday()]);
    let mut lines = csv.lines();
    assert_eq!(lines.next(), Some("start,end,seconds,class,name,category,mode"));
    let rows: Vec<&str> = lines.collect();
    assert!(rows.iter().any(|r| r.ends_with(",3600,code,code,other,")), "{csv}");
    // commas in values are quoted
    std::fs::write(s.dir.join("config.toml"), "[names]\n\"code\" = \"Editor, Inc\"\n").unwrap();
    let csv = s.ok(&["export", "-D", "2", "-d", &yesterday()]);
    assert!(csv.contains(",code,\"Editor, Inc\",other,"), "{csv}");
}

#[test]
fn bad_input_is_rejected_clearly() {
    let s = Sandbox::new("bad");
    for (args, needle) in [
        (vec!["-d", "2026-13-01"], "YYYY-MM-DD"),
        (vec!["-g", "0"], "between 0 and 24"),
        (vec!["-g", "25"], "between 0 and 24"),
        (vec!["-n", "0"], "0"),
        (vec!["nonsense"], "invalid value"),
    ] {
        let out = s.run(&args);
        assert_eq!(out.status.code(), Some(2), "{args:?} should fail as a usage error");
        let err = String::from_utf8_lossy(&out.stderr);
        assert!(err.contains(needle), "{args:?}: {err}");
    }
    // a broken config is reported and ignored, not fatal
    std::fs::write(s.dir.join("config.toml"), "this is [ not toml").unwrap();
    let out = s.run(&[]);
    assert!(out.status.success());
    assert!(String::from_utf8_lossy(&out.stderr).contains("ignoring"));
}

#[test]
fn config_backup_completions_doctor() {
    let s = Sandbox::new("misc").with_day();
    let path = s.ok(&["config"]);
    assert_eq!(path.trim(), s.dir.join("config.toml").to_string_lossy());
    assert!(
        std::fs::read_to_string(s.dir.join("config.toml")).unwrap().contains("[names]"),
        "a starter config is written"
    );
    std::fs::write(s.dir.join("config.toml"), "# mine\n").unwrap();
    s.ok(&["config"]);
    assert_eq!(
        std::fs::read_to_string(s.dir.join("config.toml")).unwrap(),
        "# mine\n",
        "an existing config is never overwritten"
    );

    let out = s.ok(&["backup"]);
    assert!(out.contains("integrity checked"), "{out}");
    let backups: Vec<_> = std::fs::read_dir(s.dir.join("backups")).unwrap().flatten().collect();
    assert_eq!(backups.len(), 1);
    let copy = Connection::open(backups[0].path()).unwrap();
    let n: i64 = copy.query_row("SELECT count(*) FROM focus", [], |r| r.get(0)).unwrap();
    let m: i64 = Connection::open(s.db())
        .unwrap()
        .query_row("SELECT count(*) FROM focus", [], |r| r.get(0))
        .unwrap();
    assert_eq!(n, m, "the backup holds every row");

    for shell in ["bash", "zsh", "fish"] {
        assert!(s.ok(&["completions", shell]).contains("focus-track"), "{shell}");
    }

    let out = s.run(&["doctor"]);
    assert_eq!(out.status.code(), Some(1), "doctor exits 1 when the recorder isn't running");
    let text = String::from_utf8_lossy(&out.stdout);
    assert!(text.contains("tracker is NOT running") && text.contains("backups: 1"), "{text}");

    use std::os::unix::fs::PermissionsExt;
    let mode = std::fs::metadata(s.db()).unwrap().permissions().mode() & 0o777;
    assert_eq!(mode, 0o600, "the database is private once focus-track has opened it");
}
