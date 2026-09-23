# Configuration

`focus-track config` creates `~/.config/focus-track/config.toml` (with comments) and prints its path.
Nothing from the config is stored with your data, so every change applies to all your history at once.

```toml
# Rename window classes. Globs (* and ?) work. Classes with the same name are merged.
[names]
"com.mitchellh.ghostty" = "Terminal"
"chromium" = "Browser"
"brave-browser" = "Browser"

# Categories: `apps` match names or classes; `sites` match the domain of browser pages.
[categories.work]
apps  = ["Terminal", "code"]
sites = ["github.com", "docs.rs"]

[categories.media]
apps  = ["mpv"]
sites = ["youtube.com"]
```

Web apps (Omarchy's Discord, WhatsApp, and so on) are named after their site automatically: `chrome-discord.com__…`
shows as **Discord**, `chrome-mail.example.org__…` as **Example Mail**, and different browser profiles are merged. Your
`[names]` rules still win. Their site also counts for `sites`, so `sites = ["discord.com"]` catches the Discord app too.
Unread counters in titles, like `(230) Discord | Friends`, are ignored so one page isn't split into many rows.

## Browsers

focus-track recognizes Chromium, Chrome, Brave, Vivaldi, Edge, Opera, Firefox, LibreWolf, Zen, Floorp and Waterfox,
including Flatpak and Snap installs, and finds their history by itself. For a browser it can't place, `focus-track
doctor` says so. Its pages are then **not recorded at all**, since a page can only be trusted to be non-private if it
appears in the browser's own history. To record them, tell it where the history lives:

```toml
[browsers]
"my-browser*" = "~/.config/my-browser/*/History"          # Chromium-style history
"other-fork"  = "~/.other-fork/*/places.sqlite"           # Firefox-style history
```

How time gets a category:

1. Browser time on a page whose domain is in some category's `sites` (subdomains included) → that category.
   So YouTube counts as *media* even if your browser is listed under *work*.
2. Otherwise the app's name or class in some category's `apps` → that category.
3. Otherwise → `other`.

It's all plain rules, applied locally. `focus-track apps` lists every class with its current name and category, and
`focus-track doctor` points out uncategorized time and renames that match nothing.
