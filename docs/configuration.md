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

How time gets a category:

1. Browser time on a page whose domain is in some category's `sites` (subdomains included) → that category.
   So YouTube counts as *media* even if your browser is listed under *work*.
2. Otherwise the app's name or class in some category's `apps` → that category.
3. Otherwise → `other`.

It's all plain rules, applied locally. `focus-track apps` lists every class with its current name and category, and
`focus-track doctor` points out uncategorized time and renames that match nothing.
