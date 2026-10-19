use crate::store::validate_name;
use crate::ui::palette::Dock;
use std::io::{self, Write};
use std::path::Path;
use std::time::Duration;

const FILE: &str = "config.toml";

/// Every key stays commented so later default changes reach users who never edited the file.
pub const DEFAULT_TOML: &str = "\
# clidraw settings. Uncomment a line to override its default.

# Idle milliseconds after an edit before the canvas saves.
# autosave_ms = 500

# Longest gap in milliseconds between two clicks that make a double click.
# double_click_ms = 400

# Screen edge of the tool palette: left, top, right or bottom.
# dock = \"left\"

# Canvas to create when none exists and no name is given.
# default_canvas = \"untitled\"
";

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Config {
    pub autosave: Duration,
    pub double_click: Duration,
    pub dock: Dock,
    pub default_canvas: String,
}

impl Default for Config {
    fn default() -> Config {
        Config {
            autosave: Duration::from_millis(500),
            double_click: Duration::from_millis(400),
            dock: Dock::Left,
            default_canvas: "untitled".into(),
        }
    }
}

impl Config {
    /// Reads `dir/config.toml`, writing the commented defaults first when it is missing.
    /// Bad input never fails: the offending setting keeps its default and a warning explains why.
    pub fn load(dir: &Path) -> (Config, Vec<String>) {
        let mut warns = vec![];
        let path = dir.join(FILE);
        if let Err(e) = write_default(dir, &path) {
            warns.push(format!("{FILE}: cannot write defaults: {e}"));
        }
        let mut cfg = Config::default();
        match std::fs::read_to_string(&path) {
            Ok(text) => cfg.apply(&text, &mut warns),
            Err(e) if e.kind() == io::ErrorKind::NotFound => {}
            Err(e) => warns.push(format!("{FILE}: {e}")),
        }
        (cfg, warns)
    }

    fn apply(&mut self, text: &str, warns: &mut Vec<String>) {
        let table = match text.parse::<toml::Table>() {
            Ok(t) => t,
            Err(e) => {
                let shown = e.to_string();
                let first = shown.lines().next().unwrap_or_default();
                let last = shown.lines().last().unwrap_or_default();
                warns.push(format!("{FILE}: {first}: {last}"));
                return;
            }
        };
        for (key, value) in &table {
            let done = match key.as_str() {
                "autosave_ms" => millis(value).map(|d| self.autosave = d),
                "double_click_ms" => millis(value).map(|d| self.double_click = d),
                "dock" => dock(value).map(|d| self.dock = d),
                "default_canvas" => name(value).map(|n| self.default_canvas = n),
                _ => {
                    warns.push(format!("{FILE}: unknown key {key}"));
                    continue;
                }
            };
            if let Err(why) = done {
                warns.push(format!("{FILE}: key {key}: {why}"));
            }
        }
    }
}

// create_new never overwrites a file the user already keeps in git.
fn write_default(dir: &Path, path: &Path) -> io::Result<()> {
    std::fs::create_dir_all(dir)?;
    match std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(path)
    {
        Ok(mut f) => f.write_all(DEFAULT_TOML.as_bytes()),
        Err(e) if e.kind() == io::ErrorKind::AlreadyExists => Ok(()),
        Err(e) => Err(e),
    }
}

fn millis(v: &toml::Value) -> Result<Duration, String> {
    match v.as_integer() {
        Some(n) if n > 0 => Ok(Duration::from_millis(n as u64)),
        _ => Err("expected a positive integer".into()),
    }
}

fn dock(v: &toml::Value) -> Result<Dock, String> {
    match v.as_str().map(str::to_lowercase).as_deref() {
        Some("left") => Ok(Dock::Left),
        Some("top") => Ok(Dock::Top),
        Some("right") => Ok(Dock::Right),
        Some("bottom") => Ok(Dock::Bottom),
        _ => Err("expected \"left\", \"top\", \"right\" or \"bottom\"".into()),
    }
}

fn name(v: &toml::Value) -> Result<String, String> {
    let s = v.as_str().ok_or("expected a string")?;
    validate_name(s).map_err(|e| e.to_string())?;
    Ok(s.to_string())
}
