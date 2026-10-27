use clidraw::config::DEFAULT_TOML;
use clidraw::ui::help::{self, SCOPES};

fn check_page(file: &str, actual: &str) {
    let path = format!("{}/docs/wiki/{file}", env!("CARGO_MANIFEST_DIR"));
    if std::env::var_os("UPDATE_DOCS").is_some() {
        std::fs::write(&path, actual).unwrap();
    }
    let expected = std::fs::read_to_string(&path).unwrap();
    assert_eq!(
        actual, expected,
        "docs page {file} differs; run UPDATE_DOCS=1 cargo test --test docs"
    );
}

fn esc(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

fn config_keys() -> String {
    let mut out = String::new();
    for block in DEFAULT_TOML.trim_end().split("\n\n").skip(1) {
        let lines: Vec<&str> = block.lines().collect();
        let (last, comment) = lines.split_last().unwrap();
        let (key, value) = last
            .strip_prefix("# ")
            .and_then(|l| l.split_once(" = "))
            .unwrap_or_else(|| panic!("no key line in block: {block}"));
        // The sample color is not the default; Config::default() leaves it unset.
        let value = if key == "chrome_accent" {
            "unset"
        } else {
            value
        };
        let text: Vec<&str> = comment.iter().map(|l| l.trim_start_matches("# ")).collect();
        out += &format!(
            "<h3 id=\"{key}\">{key}</h3>\n<p>{}</p>\n<p>Default: <code>{}</code></p>\n",
            esc(&text.join(" ")),
            esc(value)
        );
    }
    out
}

fn page(file: &str, title: &str, description: &str, body: &str) -> String {
    format!(
        r#"<!DOCTYPE html>
<html lang="en">
<head>
  <meta charset="UTF-8">
  <meta name="viewport" content="width=device-width, initial-scale=1.0">
  <title>{title} | clidraw</title>
  <meta name="description" content="{description}">
  <link rel="canonical" href="https://clidraw.shrtcts.fr/{file}">
  <meta property="og:type" content="website">
  <meta property="og:site_name" content="clidraw">
  <meta property="og:title" content="{title} | clidraw">
  <meta property="og:description" content="{description}">
  <meta property="og:url" content="https://clidraw.shrtcts.fr/{file}">
  <meta name="twitter:card" content="summary">
  <meta name="twitter:title" content="{title} | clidraw">
  <meta name="twitter:description" content="{description}">
  <script type="application/ld+json">
  {{
    "@context": "https://schema.org",
    "@type": "SoftwareApplication",
    "name": "clidraw",
    "operatingSystem": "Linux, macOS, Windows",
    "applicationCategory": "DeveloperApplication",
    "description": "{description}",
    "url": "https://clidraw.shrtcts.fr/{file}",
    "downloadUrl": "https://github.com/shortcuts/clidraw/releases",
    "offers": {{ "@type": "Offer", "price": "0", "priceCurrency": "USD" }}
  }}
  </script>
  <link rel="stylesheet" href="style.css">
</head>
<body>
<div class="layout">

  <main>
{body}  </main>

</div>
<script src="wiki-init.js"></script>
</body>
</html>
"#
    )
}

fn config_page() -> String {
    let body = format!(
        r#"    <h1 id="configuration">Configuration</h1>
    <p>clidraw reads <code>config.toml</code> from its config directory. The first run writes the file with every key commented. Run <code>clidraw config</code> to print the default file. A bad or unknown key shows a status line and keeps its default.</p>

    <h2 id="file-location">File location</h2>
    <p>clidraw uses the first of these directories that is set:</p>
    <ul>
      <li><code>$CLIDRAW_HOME</code></li>
      <li><code>$XDG_CONFIG_HOME/clidraw</code></li>
      <li><code>%APPDATA%\clidraw</code> on Windows</li>
      <li><code>~/.config/clidraw</code> on macOS and Linux</li>
    </ul>

    <h2 id="keys">Keys</h2>
{keys}
    <h2 id="default-file">Default file</h2>
    <pre><code>{default}</code></pre>
"#,
        keys = config_keys(),
        default = esc(DEFAULT_TOML),
    );
    page(
        "config.html",
        "Configuration",
        "Every clidraw setting in config.toml, with its default.",
        &body,
    )
}

fn keys_page() -> String {
    let mut body = String::from(
        "    <h1 id=\"key-bindings\">Key bindings</h1>\n    <p>Press <code>?</code> in clidraw to see the same list. Type to filter it.</p>\n",
    );
    for (i, scope) in SCOPES.iter().enumerate().take(5) {
        let slug = scope.to_lowercase().replace(' ', "-");
        body += &format!(
            "\n    <h2 id=\"{slug}\">{scope}</h2>\n    <table>\n      <tr><th>Keys</th><th>Action</th></tr>\n"
        );
        for (keys, action) in help::rows(i) {
            body += &format!(
                "      <tr><td><code>{}</code></td><td>{}</td></tr>\n",
                esc(&keys),
                esc(&action)
            );
        }
        body += "    </table>\n";
    }
    page(
        "keys.html",
        "Key bindings",
        "Every clidraw key binding, grouped by scope.",
        &body,
    )
}

#[test]
fn config_page_matches_committed() {
    check_page("config.html", &config_page());
}

#[test]
fn keys_page_matches_committed() {
    check_page("keys.html", &keys_page());
}
