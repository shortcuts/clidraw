use crate::model::HISTORY_LIMIT;
use crate::store::validate_name;
use crate::ui::theme::ThemeSetting;
use ratatui::style::Color;
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::time::Duration;

const FILE: &str = "config.toml";

/// Every key stays commented so later default changes reach users who never edited the file.
pub const DEFAULT_TOML: &str = "\
# clidraw settings. Uncomment a line to override its default.

# Idle milliseconds after an edit before the canvas saves.
# autosave_ms = 500

# Longest gap in milliseconds between two clicks that make a double click.
# double_click_ms = 400

# Canvas to create when none exists and no name is given.
# default_canvas = \"untitled\"

# Cells between snap lines for drawing, moving and resizing shapes. 1 turns snapping off.
# grid = 1

# Most undo steps kept; the oldest drop first.
# history_limit = 1000

# Blank cells around the drawing in an SVG export.
# export_padding = 2

# Glide the view for Fit all and search jumps. false snaps instantly.
# motion = true

# Preview an arrow and a copy of the selected shape when the pointer rests beside it; a click adds both. false turns it off.
# quick_connect = true

# Offer a fix for a mistake in the :mermaid editor and apply it with Tab. false turns it off.
# quick_fixes = true

# Command UI: palette opens with Ctrl-p and ignores :, vim opens with : and ignores Ctrl-p.
# command_ui = \"palette\"

# Percent of the default distance that wheel and trackpad scrolling pans. 50 halves it.
# scroll_speed = 100

# Color of the active tool, selected row and mode badge, as \"#rrggbb\". Unset uses the theme accent.
# chrome_accent = \"#5fd7ff\"

# Color palette: auto asks the terminal for its background, or force dark or light. Without COLORTERM=truecolor, colors fall back to the nearest of 256 or 16.
# theme = \"auto\"

# Folder of your Obsidian vault. Save to Obsidian note in the command palette writes into it.
# obsidian_vault = \"~/Documents/vault\"
";

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum CommandUi {
    #[default]
    Palette,
    Vim,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Config {
    pub autosave: Duration,
    pub double_click: Duration,
    pub grid: i32,
    pub default_canvas: String,
    pub history_limit: usize,
    pub export_padding: i32,
    pub motion: bool,
    pub quick_connect: bool,
    pub quick_fixes: bool,
    pub command_ui: CommandUi,
    pub scroll_speed: i32,
    pub theme: ThemeSetting,
    pub chrome_accent: Option<Color>,
    pub obsidian_vault: Option<PathBuf>,
}

impl Default for Config {
    fn default() -> Config {
        Config {
            autosave: Duration::from_millis(500),
            double_click: Duration::from_millis(400),
            grid: 1,
            default_canvas: "untitled".into(),
            history_limit: HISTORY_LIMIT,
            export_padding: 2,
            motion: true,
            quick_connect: true,
            quick_fixes: true,
            command_ui: CommandUi::default(),
            scroll_speed: 100,
            theme: ThemeSetting::Auto,
            chrome_accent: None,
            obsidian_vault: None,
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
            match self.assign(key, value) {
                None => warns.push(format!("{FILE}: unknown key {key}")),
                Some(Err(why)) => warns.push(format!("{FILE}: key {key}: {why}")),
                Some(Ok(())) => {}
            }
        }
    }

    /// `None` means the key is unknown.
    fn assign(&mut self, key: &str, value: &toml::Value) -> Option<Result<(), String>> {
        Some(match key {
            "autosave_ms" => millis(value).map(|d| self.autosave = d),
            "double_click_ms" => millis(value).map(|d| self.double_click = d),
            "grid" => grid(value).map(|n| self.grid = n),
            "default_canvas" => name(value).map(|n| self.default_canvas = n),
            "history_limit" => count(value).map(|n| self.history_limit = n),
            "export_padding" => padding(value).map(|n| self.export_padding = n),
            "motion" => flag(value).map(|b| self.motion = b),
            "quick_connect" => flag(value).map(|b| self.quick_connect = b),
            "quick_fixes" => flag(value).map(|b| self.quick_fixes = b),
            "command_ui" => command_ui(value).map(|u| self.command_ui = u),
            "scroll_speed" => speed(value).map(|n| self.scroll_speed = n),
            "chrome_accent" => hex(value).map(|c| self.chrome_accent = Some(c)),
            "obsidian_vault" => vault(value).map(|p| self.obsidian_vault = Some(p)),
            "theme" => theme(value).map(|t| self.theme = t),
            _ => return None,
        })
    }

    /// Writes `key = value` into `dir/config.toml`, touching only that key's line.
    /// `value` is TOML text, for example `4` or `"dark"`. A bad value is rejected before any write.
    pub fn save(dir: &Path, key: &str, value: &str) -> Result<(), String> {
        let parsed = (!value.contains('\n'))
            .then(|| format!("{key} = {value}").parse::<toml::Table>().ok())
            .flatten()
            .and_then(|t| t.get(key).filter(|_| t.len() == 1).cloned())
            .ok_or("expected a single valid TOML value")?;
        Config::default()
            .assign(key, &parsed)
            .ok_or_else(|| format!("unknown key {key}"))??;

        let path = dir.join(FILE);
        let text = match std::fs::read_to_string(&path) {
            Ok(t) => t,
            Err(e) if e.kind() == io::ErrorKind::NotFound => DEFAULT_TOML.to_string(),
            Err(e) => return Err(e.to_string()),
        };
        let mut lines: Vec<String> = text.lines().map(str::to_string).collect();
        let sets_key = |l: &str, commented: bool| {
            let l = l.trim_start();
            let l = match commented {
                true => l.strip_prefix('#').map(str::trim_start),
                false => Some(l),
            };
            l.and_then(|l| l.strip_prefix(key))
                .is_some_and(|rest| rest.trim_start().starts_with('='))
        };
        let at = lines
            .iter()
            .position(|l| sets_key(l, false))
            .or_else(|| lines.iter().position(|l| sets_key(l, true)));
        let line = format!("{key} = {value}");
        match at {
            Some(i) => lines[i] = line,
            None => lines.push(line),
        }
        std::fs::create_dir_all(dir).map_err(|e| e.to_string())?;
        std::fs::write(&path, lines.join("\n") + "\n").map_err(|e| e.to_string())
    }

    pub fn reset(dir: &Path) -> Result<(), String> {
        std::fs::create_dir_all(dir).map_err(|e| e.to_string())?;
        std::fs::write(dir.join(FILE), DEFAULT_TOML).map_err(|e| e.to_string())
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

fn flag(v: &toml::Value) -> Result<bool, String> {
    v.as_bool().ok_or_else(|| "expected true or false".into())
}

fn count(v: &toml::Value) -> Result<usize, String> {
    match v.as_integer().map(usize::try_from) {
        Some(Ok(n)) if n > 0 => Ok(n),
        _ => Err("expected a positive integer".into()),
    }
}

fn grid(v: &toml::Value) -> Result<i32, String> {
    match v.as_integer() {
        Some(n @ 1..=1000) => Ok(n as i32),
        _ => Err("expected an integer from 1 to 1000".into()),
    }
}

fn padding(v: &toml::Value) -> Result<i32, String> {
    match v.as_integer() {
        Some(n @ 0..=100) => Ok(n as i32),
        _ => Err("expected an integer from 0 to 100".into()),
    }
}

fn speed(v: &toml::Value) -> Result<i32, String> {
    match v.as_integer() {
        Some(n @ 1..=1000) => Ok(n as i32),
        _ => Err("expected an integer from 1 to 1000".into()),
    }
}

fn theme(v: &toml::Value) -> Result<ThemeSetting, String> {
    match v.as_str().map(str::to_lowercase).as_deref() {
        Some("auto") => Ok(ThemeSetting::Auto),
        Some("dark") => Ok(ThemeSetting::Dark),
        Some("light") => Ok(ThemeSetting::Light),
        _ => Err("expected \"auto\", \"dark\" or \"light\"".into()),
    }
}

fn command_ui(v: &toml::Value) -> Result<CommandUi, String> {
    match v.as_str().map(str::to_lowercase).as_deref() {
        Some("palette") => Ok(CommandUi::Palette),
        Some("vim") => Ok(CommandUi::Vim),
        _ => Err("expected \"palette\" or \"vim\"".into()),
    }
}

fn hex(v: &toml::Value) -> Result<Color, String> {
    let bad = || "expected a \"#rrggbb\" string".to_string();
    let digits = v
        .as_str()
        .and_then(|s| s.strip_prefix('#'))
        .ok_or_else(bad)?;
    if digits.len() != 6 || !digits.is_ascii() {
        return Err(bad());
    }
    let n = u32::from_str_radix(digits, 16).map_err(|_| bad())?;
    Ok(Color::Rgb((n >> 16) as u8, (n >> 8) as u8, n as u8))
}

pub fn expand_home(path: &str) -> String {
    let rest = match path.strip_prefix('~') {
        Some(rest) if rest.is_empty() || rest.starts_with('/') => rest,
        _ => return path.to_string(),
    };
    match std::env::home_dir() {
        Some(home) => format!("{}{rest}", home.display()),
        None => path.to_string(),
    }
}

fn vault(v: &toml::Value) -> Result<PathBuf, String> {
    let path = PathBuf::from(expand_home(v.as_str().unwrap_or_default()));
    match v.is_str() && path.is_dir() {
        true => Ok(path),
        false => Err("expected a path to an existing folder".into()),
    }
}

fn name(v: &toml::Value) -> Result<String, String> {
    let s = v.as_str().ok_or("expected a string")?;
    validate_name(s).map_err(|e| e.to_string())?;
    Ok(s.to_string())
}
