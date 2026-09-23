//! ~/.config/focus-track/config.toml: display names and categories.
//! Applied when reading, never when recording, so edits cover all history immediately.

use std::cell::RefCell;
use std::collections::HashMap;
use std::path::PathBuf;

pub const STARTER: &str = r#"# focus-track config: renames and categories apply to all history, edits show up immediately.
# See what's there with:  focus-track apps

# Rename window classes. Globs (* and ?) work. Several classes with the same name are merged into one.
[names]
# "com.mitchellh.ghostty" = "Terminal"
# "chromium" = "Browser"
# "brave-browser" = "Browser"
# "chrome-*" = "Web apps"

# Only if your browser isn't recognized (`focus-track doctor` says so): its window class and where its history
# database lives. Chromium-style "History" and Firefox-style "places.sqlite" files both work. Globs work.
# [browsers]
# "my-browser*" = "~/.config/my-browser/*/History"

# Categories. `apps` match names or classes (globs work); `sites` match the page URL of browser time
# and win over apps, so YouTube in a "work" browser still counts as media. Everything else is "other".
# [categories.work]
# apps = ["Terminal", "code", "obsidian"]
# sites = ["github.com", "docs.python.org"]
#
# [categories.media]
# apps = ["mpv"]
# sites = ["youtube.com", "netflix.com"]
"#;

pub struct Category {
    pub name: String,
    pub apps: Vec<String>,
    pub sites: Vec<String>,
}

#[derive(Default)]
pub struct Config {
    pub path: PathBuf,
    pub exists: bool,
    pub error: Option<String>,
    pub names: Vec<(String, String)>,
    pub categories: Vec<Category>,
    /// class glob -> history database glob, for browsers focus-track doesn't know
    pub browsers: Vec<(String, String)>,
    name_cache: RefCell<HashMap<String, String>>,
    cat_cache: RefCell<HashMap<(String, String), String>>,
}

static EXTRA_BROWSERS: std::sync::OnceLock<Vec<(String, String)>> = std::sync::OnceLock::new();

/// Browsers the user declared in `[browsers]`: (class glob, history database glob).
pub fn extra_browsers() -> &'static [(String, String)] {
    EXTRA_BROWSERS.get().map_or(&[], Vec::as_slice)
}

pub fn path() -> PathBuf {
    std::env::var_os("FOCUS_TRACK_CONFIG")
        .map(PathBuf::from)
        .unwrap_or_else(|| crate::util::config_dir().join("focus-track").join("config.toml"))
}

impl Config {
    pub fn load() -> Config {
        let mut cfg = Config {
            path: path(),
            ..Default::default()
        };
        match std::fs::read_to_string(&cfg.path) {
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
            Err(e) => cfg.error = Some(e.to_string()),
            Ok(text) => {
                cfg.exists = true;
                if let Err(e) = cfg.parse(&text) {
                    cfg.error = Some(e);
                    cfg.names.clear();
                    cfg.categories.clear();
                    cfg.browsers.clear();
                }
            }
        }
        if let Some(e) = &cfg.error {
            eprintln!("focus-track: ignoring {}: {}", cfg.path.display(), e.trim());
        }
        let _ = EXTRA_BROWSERS.set(cfg.browsers.clone()); // the recorder's helpers have no Config to hand
        cfg
    }

    #[cfg(test)]
    pub fn from_str(text: &str) -> Result<Config, String> {
        let mut cfg = Config::default();
        cfg.parse(text)?;
        Ok(cfg)
    }

    fn parse(&mut self, text: &str) -> Result<(), String> {
        let table: toml::Table = text.parse().map_err(|e: toml::de::Error| e.to_string())?;
        if let Some(names) = table.get("names") {
            let names = names.as_table().ok_or("[names] must be a table")?;
            for (k, v) in names {
                let v = v.as_str().ok_or(format!("names.\"{k}\" must be a string"))?;
                self.names.push((k.clone(), crate::util::sanitize(v)));
            }
        }
        if let Some(bs) = table.get("browsers") {
            let bs = bs.as_table().ok_or("[browsers] must be a table")?;
            for (class, path) in bs {
                let path = path.as_str().ok_or(format!("browsers.\"{class}\" must be a path string"))?;
                self.browsers.push((class.clone(), path.to_string()));
            }
        }
        if let Some(cats) = table.get("categories") {
            let cats = cats.as_table().ok_or("[categories] must be a table")?;
            for (name, rule) in cats {
                let rule = rule.as_table().ok_or(format!("categories.{name} must be a table"))?;
                let list = |key: &str| -> Result<Vec<String>, String> {
                    match rule.get(key) {
                        None => Ok(vec![]),
                        Some(v) => v
                            .as_array()
                            .ok_or(format!("categories.{name}.{key} must be a list"))?
                            .iter()
                            .map(|x| {
                                x.as_str()
                                    .map(str::to_string)
                                    .ok_or(format!("categories.{name}.{key} must hold strings"))
                            })
                            .collect(),
                    }
                };
                self.categories.push(Category {
                    name: crate::util::sanitize(name),
                    apps: list("apps")?,
                    sites: list("sites")?,
                });
            }
        }
        Ok(())
    }

    /// Display name for a window class: the first matching rename, else a short form of the class.
    pub fn name_of(&self, app: &str) -> String {
        if let Some(n) = self.name_cache.borrow().get(app) {
            return n.clone();
        }
        let n = self.names.iter().find(|(p, _)| glob_match(p, app)).map_or_else(
            || webapp_host(app).map_or_else(|| short(app), |h| webapp_name(&h)),
            |(_, n)| n.clone(),
        );
        self.name_cache.borrow_mut().insert(app.to_string(), n.clone());
        n
    }

    /// Category for time in `app` (on page `url`, for browsers). Sites win over apps; default "other".
    pub fn category_of(&self, app: &str, url: &str) -> String {
        // a web app has no page URL of its own, but its window class names its site
        let h = if url.is_empty() {
            webapp_host(app).unwrap_or_default()
        } else {
            host(url)
        };
        let key = (app.to_string(), h.clone());
        if let Some(c) = self.cat_cache.borrow().get(&key) {
            return c.clone();
        }
        let site_hit = |c: &&Category| {
            !h.is_empty()
                && c.sites.iter().any(|s| {
                    let s = s.to_lowercase();
                    h == s || h.ends_with(&format!(".{s}"))
                })
        };
        let name = self.name_of(app);
        let c = self
            .categories
            .iter()
            .find(site_hit)
            .or_else(|| {
                self.categories
                    .iter()
                    .find(|c| c.apps.iter().any(|p| glob_match(p, &name) || glob_match(p, app)))
            })
            .map_or_else(|| "other".to_string(), |c| c.name.clone());
        self.cat_cache.borrow_mut().insert(key, c.clone());
        c
    }
}

/// Case-insensitive glob with `*` and `?`.
pub fn glob_match(pattern: &str, s: &str) -> bool {
    let p: Vec<char> = pattern.to_lowercase().chars().collect();
    let t: Vec<char> = s.to_lowercase().chars().collect();
    let (mut pi, mut ti, mut star, mut mark) = (0, 0, None, 0);
    while ti < t.len() {
        if pi < p.len() && (p[pi] == '?' || p[pi] == t[ti]) {
            pi += 1;
            ti += 1;
        } else if pi < p.len() && p[pi] == '*' {
            star = Some(pi);
            mark = ti;
            pi += 1;
        } else if let Some(sp) = star {
            pi = sp + 1;
            mark += 1;
            ti = mark;
        } else {
            return false;
        }
    }
    while pi < p.len() && p[pi] == '*' {
        pi += 1;
    }
    pi == p.len()
}

/// The site of a Chromium app-mode window (Omarchy web apps): `chrome-discord.com__channels_@me-Default` -> `discord.com`.
pub fn webapp_host(app: &str) -> Option<String> {
    let rest = ["chrome-", "brave-", "msedge-"].iter().find_map(|p| app.strip_prefix(p))?;
    let (host, _) = rest.split_once("__")?;
    let valid = host.contains('.') && host.chars().all(|c| c.is_ascii_alphanumeric() || c == '.' || c == '-');
    valid.then(|| host.to_lowercase())
}

/// A readable name for a site: discord.com -> Discord, web.whatsapp.com -> WhatsApp, mail.example.org -> Example Mail.
pub fn webapp_name(host: &str) -> String {
    let mut labels: Vec<&str> = host.split('.').collect();
    let n = labels.len();
    if n > 2 && labels[n - 1].len() == 2 && ["co", "com", "org", "net", "ac", "gov"].contains(&labels[n - 2]) {
        labels.pop(); // co.uk, com.au
    }
    if labels.len() > 1 {
        labels.pop(); // the TLD
    }
    while labels.len() > 1 && ["www", "web", "app", "m"].contains(&labels[0]) {
        labels.remove(0);
    }
    labels.reverse(); // mail.example -> Example Mail
    labels.iter().map(|l| brand(l)).collect::<Vec<_>>().join(" ")
}

fn brand(label: &str) -> String {
    match label {
        "whatsapp" => "WhatsApp".into(),
        "youtube" => "YouTube".into(),
        "github" => "GitHub".into(),
        "chatgpt" => "ChatGPT".into(),
        "linkedin" => "LinkedIn".into(),
        "openai" => "OpenAI".into(),
        _ => {
            let mut c = label.chars();
            c.next()
                .map_or_else(String::new, |f| f.to_uppercase().collect::<String>() + c.as_str())
        }
    }
}

/// com.mitchellh.ghostty -> ghostty, org.telegram.desktop -> telegram
pub fn short(app: &str) -> String {
    let parts: Vec<&str> = app.split('.').collect();
    if parts.len() < 3 {
        return app.to_string();
    }
    let last = parts[parts.len() - 1];
    if ["desktop", "app", "client"].contains(&last.to_lowercase().as_str()) {
        parts[parts.len() - 2].to_string()
    } else {
        last.to_string()
    }
}

/// https://www.YouTube.com:443/x -> youtube.com
pub fn host(url: &str) -> String {
    let rest = match url.find("://") {
        Some(i) if i > 0 && url[..i].chars().all(|c| c.is_alphanumeric() || c == '_') => &url[i + 3..],
        _ => url,
    };
    let h = rest.split('/').next().unwrap_or("").split(':').next().unwrap_or("").to_lowercase();
    h.strip_prefix("www.").map_or(h.clone(), str::to_string)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn names_and_categories() {
        let cfg = Config::from_str(
            r#"
            [names]
            "com.mitchellh.ghostty" = "Terminal"
            "chrom*" = "Browser"
            "brave-browser" = "Browser"
            [categories.media]
            apps = ["mpv"]
            sites = ["youtube.com"]
            [categories.work]
            apps = ["Terminal", "Browser"]
            "#,
        )
        .unwrap();
        assert_eq!(cfg.name_of("com.mitchellh.ghostty"), "Terminal");
        assert_eq!(cfg.name_of("chromium"), "Browser");
        assert_eq!(cfg.name_of("brave-browser"), "Browser");
        assert_eq!(cfg.name_of("org.gnome.Nautilus"), "Nautilus");
        assert_eq!(cfg.category_of("chromium", "https://m.youtube.com/watch?v=1"), "media");
        assert_eq!(cfg.category_of("chromium", "https://github.com"), "work");
        assert_eq!(cfg.category_of("imv", ""), "other");
        assert_eq!(cfg.category_of("mpv", ""), "media");
    }

    #[test]
    fn browsers_table() {
        let cfg = Config::from_str("[browsers]\n\"my-browser*\" = \"~/.config/my-browser/*/History\"").unwrap();
        assert_eq!(
            cfg.browsers,
            [("my-browser*".to_string(), "~/.config/my-browser/*/History".to_string())]
        );
        assert!(Config::from_str("[browsers]\nx = 3").is_err());
    }

    #[test]
    fn bad_config_is_an_error() {
        assert!(Config::from_str("[names]\nx = 3").is_err());
        assert!(Config::from_str("[categories.work]\napps = \"Terminal\"").is_err());
        assert!(Config::from_str("not toml [").is_err());
        assert!(Config::from_str(STARTER).is_ok());
    }

    #[test]
    fn helpers() {
        assert!(glob_match("chrome-*", "chrome-youtube.com__-Default"));
        assert!(glob_match("LIBRE?FFICE", "libreoffice"));
        assert!(!glob_match("code", "vscode"));
        assert_eq!(short("com.mitchellh.ghostty"), "ghostty");
        assert_eq!(short("org.telegram.desktop"), "telegram");
        assert_eq!(host("https://www.YouTube.com:443/x"), "youtube.com");
        assert_eq!(host(""), "");
    }

    #[test]
    fn web_apps_get_the_name_of_their_site() {
        for (class, host, name) in [
            ("chrome-discord.com__channels_@me-Default", "discord.com", "Discord"),
            ("chrome-discord.com__channels_@me-Profile_1", "discord.com", "Discord"),
            ("chrome-web.whatsapp.com__-Default", "web.whatsapp.com", "WhatsApp"),
            ("brave-mail.example.org__u_0_inbox-Default", "mail.example.org", "Example Mail"),
            ("chrome-docs.google.com__document-Default", "docs.google.com", "Google Docs"),
            ("chrome-www.bbc.co.uk__news-Default", "www.bbc.co.uk", "Bbc"),
            ("chrome-x.com__home-Default", "x.com", "X"),
            ("chrome-chatgpt.com__-Default", "chatgpt.com", "ChatGPT"),
        ] {
            assert_eq!(webapp_host(class).as_deref(), Some(host), "{class}");
            assert_eq!(webapp_name(host), name, "{class}");
        }
        for not_a_webapp in [
            "chromium",
            "brave-browser",
            "chrome-extension",
            "com.mitchellh.ghostty",
            "chrome-nodot__x-Default",
            "chrome-__x",
        ] {
            assert_eq!(webapp_host(not_a_webapp), None, "{not_a_webapp}");
        }
        let cfg = Config::from_str("[names]\n\"chrome-discord.com*\" = \"Chat\"\n[categories.chat]\nsites = [\"discord.com\"]\n[categories.web]\napps = [\"WhatsApp\"]").unwrap();
        assert_eq!(cfg.name_of("chrome-discord.com__x-Default"), "Chat", "your own rename wins");
        assert_eq!(cfg.name_of("chrome-web.whatsapp.com__-Default"), "WhatsApp");
        assert_eq!(
            cfg.category_of("chrome-discord.com__x-Default", ""),
            "chat",
            "the site in the class counts as a `sites` match"
        );
        assert_eq!(
            cfg.category_of("chrome-web.whatsapp.com__-Default", ""),
            "web",
            "and the derived name as an `apps` match"
        );
    }
}
