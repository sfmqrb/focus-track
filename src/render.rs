//! btop-style terminal drawing in the current Omarchy theme's colors (plain ANSI colors elsewhere).

use crate::util::rhe;
use std::path::PathBuf;
use std::sync::OnceLock;
use std::sync::atomic::{AtomicBool, Ordering};
use unicode_width::UnicodeWidthStr;

static TTY: AtomicBool = AtomicBool::new(false);

pub fn set_tty(on: bool) {
    TTY.store(on, Ordering::Relaxed);
}

pub fn tty() -> bool {
    TTY.load(Ordering::Relaxed)
}

pub struct Theme {
    pub app: Vec<String>,
    pub hex: Vec<String>,
    pub accent: String,
    pub accent_hex: String,
    pub border: String,
    pub dim: String,
    pub text: String,
    pub heat: Vec<String>,
    pub select: String,
}

/// (theme color name, ANSI fallback, hex fallback) for apps, in rank order.
const PALETTE: [(&str, &str, &str); 12] = [
    ("blue", "34", "#7aa2f7"),
    ("green", "32", "#9ece6a"),
    ("magenta", "35", "#bb9af7"),
    ("cyan", "36", "#7dcfff"),
    ("yellow", "33", "#e0af68"),
    ("red", "31", "#f7768e"),
    ("orange", "91", "#ff9e64"), // hues that differ from the first six before any bright repeats
    ("bright_cyan", "96", "#a4daff"),
    ("bright_magenta", "95", "#c7a9ff"),
    ("bright_green", "92", "#b9f27c"),
    ("bright_blue", "94", "#8db0ff"),
    ("bright_yellow", "93", "#ffc777"),
];

static THEME: OnceLock<Theme> = OnceLock::new();

pub fn theme() -> &'static Theme {
    THEME.get_or_init(load_theme)
}

fn is_hex(h: &str) -> bool {
    h.len() == 7 && h.starts_with('#') && h[1..].chars().all(|c| c.is_ascii_hexdigit())
}

fn rgb(h: &str) -> (f64, f64, f64) {
    let p = |i: usize| f64::from(u8::from_str_radix(&h[i..i + 2], 16).unwrap_or(0));
    (p(1), p(3), p(5))
}

fn truecolor(h: &str) -> String {
    let (r, g, b) = rgb(h);
    format!("38;2;{};{};{}", r as u8, g as u8, b as u8)
}

fn blend(a: &str, b: &str, t: f64) -> String {
    let ((r1, g1, b1), (r2, g2, b2)) = (rgb(a), rgb(b));
    let mix = |x: f64, y: f64| (x + (y - x) * t) as u8;
    format!("38;2;{};{};{}", mix(r1, r2), mix(g1, g2), mix(b1, b2))
}

fn load_theme() -> Theme {
    let path = std::env::var_os("FOCUS_TRACK_THEME")
        .map(PathBuf::from)
        .unwrap_or_else(|| crate::util::home().join(".local/state/omarchy/current/theme/colors.toml"));
    let colors: toml::Table = std::fs::read_to_string(path).ok().and_then(|t| t.parse().ok()).unwrap_or_default();
    // only #rrggbb values are used, so a theme file can't smuggle escape sequences into the output
    let hex = |n: &str| colors.get(n).and_then(|v| v.as_str()).filter(|h| is_hex(h)).map(str::to_string);
    let fg = |n: &str, fallback: &str| hex(n).map_or_else(|| fallback.to_string(), |h| truecolor(&h));
    let heat = match (hex("lighter_background"), hex("accent")) {
        (Some(a), Some(b)) => [0.3, 0.55, 0.8, 1.0].iter().map(|t| blend(&a, &b, *t)).collect(),
        _ => ["34;2", "34", "94", "96"].map(String::from).to_vec(),
    };
    Theme {
        app: PALETTE.iter().map(|(n, c, _)| fg(n, c)).collect(),
        hex: PALETTE.iter().map(|(n, _, h)| hex(n).unwrap_or_else(|| h.to_string())).collect(),
        accent: fg("accent", "34"),
        accent_hex: hex("accent").unwrap_or_else(|| PALETTE[0].2.to_string()),
        border: fg("muted", "90"),
        dim: fg("dark_foreground", "2"),
        text: fg("foreground", "39"),
        heat,
        // the selected row: a background band between the panel color and the accent, like btop
        select: match (hex("lighter_background"), hex("accent")) {
            (Some(a), Some(b)) => blend(&a, &b, 0.35).replacen("38;", "48;", 1),
            _ => "48;5;238".into(),
        },
    }
}

pub fn paint(code: &str, s: &str) -> String {
    if tty() && !s.is_empty() {
        format!("\x1b[{code}m{s}\x1b[0m")
    } else {
        s.to_string()
    }
}

pub fn app_code(i: usize) -> String {
    let th = theme();
    th.app.get(i).unwrap_or(&th.dim).clone() // past the palette: dim "other"
}

pub fn color(i: usize, s: &str) -> String {
    paint(&app_code(i), s)
}

pub fn dim(s: &str) -> String {
    paint(&theme().dim, s)
}

/// Clickable text in terminals that support OSC 8 hyperlinks.
pub fn link(url: &str, text: &str) -> String {
    if tty() && !url.is_empty() {
        format!("\x1b]8;;{url}\x1b\\{text}\x1b]8;;\x1b\\")
    } else {
        text.to_string()
    }
}

/// Split into (is_escape_sequence, text) pieces: CSI sequences and OSC sequences are escapes.
fn pieces(s: &str) -> Vec<(bool, String)> {
    let mut out = Vec::new();
    let mut it = s.chars().peekable();
    while let Some(c) = it.next() {
        if c != '\x1b' {
            out.push((false, c.to_string()));
            continue;
        }
        let mut esc = String::from(c);
        match it.peek() {
            Some('[') => {
                esc.push(it.next().unwrap_or('['));
                for x in it.by_ref() {
                    esc.push(x);
                    if ('@'..='~').contains(&x) {
                        break;
                    }
                }
            }
            Some(']') => {
                esc.push(it.next().unwrap_or(']'));
                while let Some(x) = it.next() {
                    esc.push(x);
                    if x == '\x07' {
                        break;
                    }
                    if x == '\x1b' {
                        if it.peek() == Some(&'\\') {
                            esc.push(it.next().unwrap_or('\\'));
                        }
                        break;
                    }
                }
            }
            _ => {}
        }
        out.push((true, esc));
    }
    out
}

pub fn strip_ansi(s: &str) -> String {
    pieces(s).into_iter().filter(|p| !p.0).map(|p| p.1).collect()
}

/// Width on screen.
pub fn vlen(s: &str) -> usize {
    strip_ansi(s).width()
}

/// Pad or cut a colored string to exactly w columns.
pub fn fit(s: &str, w: usize) -> String {
    let n = vlen(s);
    if n <= w {
        return format!("{s}{}", " ".repeat(w - n));
    }
    // The text kept so far is measured as a whole, never one character at a time: terminals draw emoji
    // with variation selectors, skin tones, joiners and flags as one glyph, so their cells are not the sum
    // of the characters'. Text stops at the first character that would overflow.
    let (mut out, mut kept, mut full, mut in_link) = (String::new(), String::new(), false, false);
    for (esc, text) in pieces(s) {
        if esc {
            if text.starts_with("\x1b]8;;") {
                in_link = !text.starts_with("\x1b]8;;\x1b");
            }
            out.push_str(&text); // colors and links are always kept, so they close properly
            continue;
        }
        if full {
            continue;
        }
        kept.push_str(&text);
        if kept.width() > w {
            kept.truncate(kept.len() - text.len());
            full = true;
        } else {
            out.push_str(&text);
        }
    }
    out.push_str(&" ".repeat(w - kept.width()));
    if tty() {
        out.push_str("\x1b[0m");
        if in_link {
            out.push_str("\x1b]8;;\x1b\\");
        }
    }
    out
}

/// ╭─┤title├──╮ panel of width w (and height h); the active panel gets an accent border.
pub fn boxed(title: &str, lines: &[String], w: usize, h: Option<usize>, hint: &str, active: bool) -> Vec<String> {
    let th = theme();
    let h = h.unwrap_or(lines.len() + 2);
    let edge = if active { &th.accent } else { &th.border };
    let title = fit(title, w.saturating_sub(6)).trim_end().to_string();
    let top = paint(edge, "╭─┤")
        + &paint(&format!("1;{}", th.accent), &title)
        + &paint(edge, &format!("├{}╮", "─".repeat(w.saturating_sub(vlen(&title) + 5))));
    let hint = if !hint.is_empty() && vlen(hint) + 8 <= w {
        format!(" {hint} ")
    } else {
        String::new()
    };
    let bottom = paint(edge, &format!("╰{}", "─".repeat(w.saturating_sub(vlen(&hint) + 4)))) + &dim(&hint) + &paint(edge, "──╯");
    let side = paint(edge, "│");
    let mut out = vec![top];
    for i in 0..h.saturating_sub(2) {
        let line = lines.get(i).map_or("", String::as_str);
        out.push(format!("{side} {} {side}", fit(line, w.saturating_sub(4))));
    }
    out.push(bottom);
    out
}

pub fn hstack(boxes: &[Vec<String>]) -> Vec<String> {
    let n = boxes.iter().map(Vec::len).min().unwrap_or(0);
    (0..n).map(|i| boxes.iter().map(|b| b[i].as_str()).collect()).collect()
}

/// btop-style ■■■■■····· meter.
pub fn meter(frac: f64, width: usize, code: &str) -> String {
    let f = if frac.is_finite() { frac.clamp(0.0, 1.0) } else { 0.0 };
    let n = (rhe(f * width as f64) as usize).min(width);
    paint(code, &"■".repeat(n)) + &paint(&theme().border, &"■".repeat(width - n))
}

/// Visual language, used the same way everywhere:
///   ■ meter  = how much time      ● dot = you were there (a minute / an hour)
///   · dot    = nothing there      braille = a trend over the day      reversed row = selected
pub const DOT: &str = "●";
pub const EMPTY: &str = "·";

/// Color swatches for a chart (legends always use the dot), wrapped onto as many lines as `w` needs.
pub fn legend_lines(names: &[String], w: usize) -> Vec<String> {
    let mut lines: Vec<String> = vec![String::new()];
    for (i, a) in names.iter().enumerate() {
        let item = color(i, &format!("{DOT} ")) + a;
        let last = lines.last_mut().expect("never empty");
        if !last.is_empty() && vlen(last) + 2 + vlen(&item) > w {
            lines.push(item);
        } else {
            *last = if last.is_empty() { item } else { format!("{last}  {item}") };
        }
    }
    lines
}

/// The selected row of any list: a background band across the whole line; the row keeps its own colors,
/// so meters still show how much.
pub fn highlight(line: &str, w: usize) -> String {
    if !tty() {
        return fit(line, w);
    }
    let band = format!("\x1b[{}m", theme().select);
    format!("{band}\x1b[1m{}\x1b[0m", fit(line, w).replace("\x1b[0m", &format!("\x1b[0m{band}")))
}

/// "key what  key what" with keys in the accent color. The last two pairs (help, quit/back) always
/// show; the others are dropped from the end when the terminal is too narrow.
pub fn hints(pairs: &[(&str, &str)], w: usize) -> String {
    let th = theme();
    let one = |(k, what): &(&str, &str)| format!("{} {}", paint(&format!("1;{}", th.accent), k), dim(what));
    let width = |ps: &[&(&str, &str)]| {
        ps.iter()
            .map(|(k, what)| k.chars().count() + what.chars().count() + 3)
            .sum::<usize>()
            + 1
    };
    let (optional, essential) = pairs.split_at(pairs.len().saturating_sub(2));
    let mut shown: Vec<&(&str, &str)> = essential.iter().collect();
    for i in 0..optional.len() {
        let mut trial: Vec<&(&str, &str)> = optional[..=i].iter().collect();
        trial.extend(essential.iter());
        if width(&trial) > w {
            break;
        }
        shown = trial;
    }
    fit(&format!(" {}", shown.iter().map(|p| one(p)).collect::<Vec<_>>().join("  ")), w)
}

/// btop-style filled graph: each value 0-1 is one dot column, 2 per character, 4 dot levels per row.
pub fn braille(vals: &[f64], rows: usize) -> Vec<String> {
    let mut levels: Vec<usize> = vals
        .iter()
        .map(|v| {
            if *v > 0.0 {
                (rhe(v.min(1.0) * rows as f64 * 4.0) as usize).max(1)
            } else {
                0
            }
        })
        .collect();
    if levels.len() % 2 == 1 {
        levels.push(0);
    }
    const LEFT: [u32; 4] = [0x40, 0x04, 0x02, 0x01]; // dot bits, bottom to top
    const RIGHT: [u32; 4] = [0x80, 0x20, 0x10, 0x08];
    (0..rows)
        .map(|r| {
            let below = (rows - 1 - r) * 4;
            levels
                .chunks(2)
                .map(|pair| {
                    let mut ch = 0;
                    for (level, bits) in [(pair[0], LEFT), (pair[1], RIGHT)] {
                        for b in bits.iter().take(level.saturating_sub(below).min(4)) {
                            ch |= b;
                        }
                    }
                    char::from_u32(0x2800 + ch).unwrap_or(' ')
                })
                .collect()
        })
        .collect()
}

/// braille() with a color per character cell: code_at(row, char_index).
pub fn braille_colored(vals: &[f64], rows: usize, code_at: &dyn Fn(usize, usize) -> String) -> Vec<String> {
    braille(vals, rows)
        .iter()
        .enumerate()
        .map(|(r, line)| {
            let (mut s, mut run, mut cur) = (String::new(), String::new(), String::new());
            for (c, ch) in line.chars().enumerate() {
                let code = code_at(r, c);
                if code != cur && !run.is_empty() {
                    s += &paint(&cur, &run);
                    run.clear();
                }
                cur = code;
                run.push(ch);
            }
            s + &paint(&cur, &run)
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn widths() {
        set_tty(true);
        assert_eq!(vlen(&fit("\x1b[1mabcdef\x1b[0m", 3)), 3);
        assert_eq!(fit("ab", 4), "ab  ");
        let osc = "\x1b]8;;https://x.io\x1b\\page\x1b]8;;\x1b\\";
        assert_eq!(vlen(osc), 4);
        assert_eq!(strip_ansi(&fit(&format!("{osc}tail"), 6)), "pageta");
        let cut = fit(&link("https://x.io", "a long page title"), 5);
        assert_eq!(cut.matches("\x1b]8;;").count(), 2, "a cut link is still closed");
        assert_eq!(vlen("日本語"), 6);
        assert_eq!(vlen(&fit("日本語", 5)), 5); // wide chars never overflow
        assert!(
            boxed(
                "t",
                &["x".repeat(50), "y".into()],
                30,
                None,
                "a very long hint that does not fit",
                false
            )
            .iter()
            .all(|l| vlen(l) == 30)
        );
        assert!(
            boxed("a title far too long for this box", &[], 12, Some(4), "", true)
                .iter()
                .all(|l| vlen(l) == 12)
        );
        assert_eq!(strip_ansi(&meter(0.5, 4, "34")), "■■■■");
    }

    #[test]
    fn legends_wrap_instead_of_cutting() {
        set_tty(true);
        let names: Vec<String> = (0..10).map(|i| format!("application-{i}")).collect();
        let lines = legend_lines(&names, 40);
        assert!(lines.len() > 1 && lines.iter().all(|l| vlen(l) <= 40));
        let all = lines.iter().map(|l| strip_ansi(l)).collect::<Vec<_>>().join(" ");
        assert!(names.iter().all(|n| all.contains(n.as_str())), "every name is shown");
        assert_eq!(legend_lines(&[], 40), vec![String::new()]);
    }

    /// Text that terminals draw in a different number of cells than a per-character count suggests:
    /// emoji with variation selectors, skin tones, joined emoji, flags, keycaps, direction marks, Arabic script.
    const TRICKY: &[&str] = &[
        "❤️ A sample song title with a heart #song",
        "Rematch is coming soon ☠️🔥 #sports #shorts",
        "Two friends arm wrestling 🤲🏻❤️ - YouTube",
        "👰\u{200d}♀️👈 bride and friends #shorts #love",
        "🇺🇸 flags 🇵🇸 and keycaps 1️⃣2️⃣3️⃣ in one title",
        "(4) \u{200e}\u{2068}name here\u{2069} @ \u{200e}\u{2068}other\u{2069} (123)",
        "سلام دنیا — LibreOffice Writer",
        "日本語のタイトルとemoji😂😂😂 mixed 中文",
        "e\u{301}\u{301} combining marks and Zalgo t\u{338}\u{338}ext",
    ];

    #[test]
    fn fit_is_exact_for_text_that_terminals_draw_oddly() {
        set_tty(true);
        for s in TRICKY {
            for w in 0..80 {
                let f = fit(s, w);
                assert_eq!(vlen(&f), w, "fit({s:?}, {w}) is {} cells wide: {:?}", vlen(&f), strip_ansi(&f));
            }
            // cutting only ever shortens: what's left is the start of the original
            let cut = strip_ansi(&fit(s, 12)).trim_end().to_string();
            assert!(s.starts_with(&cut) || cut.is_empty(), "{s:?} -> {cut:?}");
        }
        // pseudo-random mixes of all of it, inside colors and links, must also come out exact
        let atoms = [
            "a",
            "Z",
            " ",
            "é",
            "❤\u{fe0f}",
            "☠\u{fe0f}",
            "🤲\u{1f3fb}",
            "👰\u{200d}♀\u{fe0f}",
            "😂",
            "🇺🇸",
            "1\u{fe0f}\u{20e3}",
            "日",
            "ب",
            "\u{200e}",
            "\u{2068}",
            "\u{301}",
        ];
        let mut seed = 0x9e37_79b9_u64;
        let mut next = |n: usize| {
            seed ^= seed << 13;
            seed ^= seed >> 7;
            seed ^= seed << 17;
            (seed % n as u64) as usize
        };
        for _ in 0..400 {
            let text: String = (0..next(30)).map(|_| atoms[next(atoms.len())]).collect();
            let styled = match next(3) {
                0 => text.clone(),
                1 => paint("1;34", &text),
                _ => link("https://example.org/x", &paint("2", &text)),
            };
            let w = next(40);
            let f = fit(&styled, w);
            assert_eq!(vlen(&f), w, "fit({styled:?}, {w}) -> {:?}", strip_ansi(&f));
            let boxed_line = &boxed("t", &[styled], 44, None, "", false)[1];
            assert_eq!(vlen(boxed_line), 44, "{:?}", strip_ansi(boxed_line));
        }
    }

    #[test]
    fn braille_dots() {
        assert_eq!(braille(&[1.0, 0.0], 1), vec!["⡇"]);
        assert_eq!(braille(&[0.0, 1.0], 1), vec!["⢸"]);
        assert_eq!(braille(&[0.5, 0.0], 2), vec!["⠀", "⡇"]);
        assert_eq!(braille(&[1.0], 1), vec!["⡇"]);
    }
}
