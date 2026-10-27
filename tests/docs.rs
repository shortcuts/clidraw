use clidraw::config::DEFAULT_TOML;

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

fn config_page() -> String {
    format!(
        r#"<!DOCTYPE html>
<html lang="en">
<head>
  <meta charset="UTF-8">
  <meta name="viewport" content="width=device-width, initial-scale=1.0">
  <title>Configuration | clidraw</title>
  <meta name="description" content="Every clidraw setting in config.toml, with its default.">
  <link rel="canonical" href="https://clidraw.shrtcts.fr/config.html">
  <meta property="og:type" content="website">
  <meta property="og:site_name" content="clidraw">
  <meta property="og:title" content="Configuration | clidraw">
  <meta property="og:description" content="Every clidraw setting in config.toml, with its default.">
  <meta property="og:url" content="https://clidraw.shrtcts.fr/config.html">
  <meta name="twitter:card" content="summary">
  <meta name="twitter:title" content="Configuration | clidraw">
  <meta name="twitter:description" content="Every clidraw setting in config.toml, with its default.">
  <script type="application/ld+json">
  {{
    "@context": "https://schema.org",
    "@type": "SoftwareApplication",
    "name": "clidraw",
    "operatingSystem": "Linux, macOS, Windows",
    "applicationCategory": "DeveloperApplication",
    "description": "Every clidraw setting in config.toml, with its default.",
    "url": "https://clidraw.shrtcts.fr/config.html",
    "downloadUrl": "https://github.com/shortcuts/clidraw/releases",
    "offers": {{ "@type": "Offer", "price": "0", "priceCurrency": "USD" }}
  }}
  </script>
  <link rel="stylesheet" href="style.css">
</head>
<body>
<div class="layout">

  <main>
    <h1 id="configuration">Configuration</h1>
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
  </main>

</div>
<script src="wiki-init.js"></script>
</body>
</html>
"#,
        keys = config_keys(),
        default = esc(DEFAULT_TOML),
    )
}

#[test]
fn config_page_matches_committed() {
    check_page("config.html", &config_page());
}
