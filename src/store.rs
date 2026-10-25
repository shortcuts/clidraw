use crate::model::{Canvas, Command, ModelError, VERSION};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::fmt;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

const EXT: &str = ".clidraw.json";

/// The save-file text; export writes the same bytes.
pub fn encode(canvas: &Canvas) -> Result<String, StoreError> {
    let mut text = serde_json::to_string_pretty(canvas)?;
    text.push('\n');
    Ok(text)
}

#[derive(Debug)]
pub enum StoreError {
    BadName(String),
    Exists(String),
    TooNew(u32),
    Io(io::Error),
    Json(serde_json::Error),
    Invalid(ModelError),
}

impl fmt::Display for StoreError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            StoreError::BadName(n) => write!(f, "bad canvas name {n:?}"),
            StoreError::Exists(n) => write!(f, "canvas {n:?} exists"),
            StoreError::TooNew(v) => write!(f, "canvas file has version {v}, newer than {VERSION}"),
            StoreError::Io(e) => write!(f, "{e}"),
            StoreError::Json(e) => write!(f, "{e}"),
            StoreError::Invalid(e) => write!(f, "invalid canvas: {e}"),
        }
    }
}

impl std::error::Error for StoreError {}

impl From<io::Error> for StoreError {
    fn from(e: io::Error) -> Self {
        StoreError::Io(e)
    }
}

impl From<serde_json::Error> for StoreError {
    fn from(e: serde_json::Error) -> Self {
        StoreError::Json(e)
    }
}

/// Volatile data kept out of the git-tracked canvas files.
#[derive(Default, Serialize, Deserialize)]
pub struct State {
    pub last: Option<String>,
    pub offsets: BTreeMap<String, (i32, i32)>,
}

pub struct Store {
    dir: PathBuf,
    state_dir: PathBuf,
}

#[derive(Serialize, Deserialize)]
struct HistoryFile {
    canvas_hash: u64,
    undo: Vec<Command>,
}

#[derive(Deserialize)]
struct VersionOnly {
    version: u32,
}

/// Picks the canvas directory. macOS uses ~/.config because dotfile repos manage it.
pub fn resolve_dir(env: impl Fn(&str) -> Option<String>, home: Option<PathBuf>) -> Option<PathBuf> {
    let set = |k: &str| env(k).filter(|v| !v.is_empty());
    if let Some(d) = set("CLIDRAW_HOME") {
        return Some(PathBuf::from(d));
    }
    if let Some(d) = set("XDG_CONFIG_HOME") {
        return Some(Path::new(&d).join("clidraw"));
    }
    if cfg!(windows) {
        return set("APPDATA").map(|d| Path::new(&d).join("clidraw"));
    }
    home.map(|h| h.join(".config").join("clidraw"))
}

pub fn resolve_state_dir(
    env: impl Fn(&str) -> Option<String>,
    home: Option<PathBuf>,
) -> Option<PathBuf> {
    let set = |k: &str| env(k).filter(|v| !v.is_empty());
    if let Some(d) = set("XDG_STATE_HOME") {
        return Some(Path::new(&d).join("clidraw"));
    }
    if cfg!(windows) {
        return set("LOCALAPPDATA").map(|d| Path::new(&d).join("clidraw"));
    }
    home.map(|h| h.join(".local").join("state").join("clidraw"))
}

pub fn validate_name(name: &str) -> Result<(), StoreError> {
    let bad = name.is_empty() || name.starts_with('.') || name.contains(['/', '\\', '\0']);
    if bad {
        Err(StoreError::BadName(name.to_string()))
    } else {
        Ok(())
    }
}

impl Store {
    pub fn at(dir: PathBuf, state_dir: PathBuf) -> Store {
        Store { dir, state_dir }
    }

    /// Snippets share the canvas file format.
    pub fn library(&self) -> Store {
        Store::at(self.dir.join("library"), self.state_dir.clone())
    }

    pub fn dir(&self) -> &Path {
        &self.dir
    }

    pub fn from_env() -> Result<Store, StoreError> {
        let home = directories::BaseDirs::new().map(|b| b.home_dir().to_path_buf());
        let env = |k: &str| std::env::var(k).ok();
        let missing = || io::Error::other("cannot find a home directory");
        let dir = resolve_dir(env, home.clone()).ok_or_else(missing)?;
        let state = resolve_state_dir(env, home).ok_or_else(missing)?;
        Ok(Store::at(dir, state))
    }

    fn path(&self, name: &str) -> Result<PathBuf, StoreError> {
        validate_name(name)?;
        Ok(self.dir.join(format!("{name}{EXT}")))
    }

    pub fn exists(&self, name: &str) -> bool {
        self.path(name).is_ok_and(|p| p.exists())
    }

    /// Newest first. Also removes temp files that a crash left behind.
    pub fn list(&self) -> Result<Vec<String>, StoreError> {
        let mut found = vec![];
        let Ok(rd) = fs::read_dir(&self.dir) else {
            return Ok(vec![]);
        };
        for entry in rd.flatten() {
            let file = entry.file_name().to_string_lossy().into_owned();
            if let Some(name) = file.strip_suffix(EXT).filter(|n| !n.starts_with('.')) {
                let mtime = entry.metadata().and_then(|m| m.modified()).ok();
                found.push((mtime, name.to_string()));
            } else if file.starts_with('.') && file.ends_with(&format!("{EXT}.tmp")) {
                let _ = fs::remove_file(entry.path());
            }
        }
        found.sort_by(|a, b| b.cmp(a));
        Ok(found.into_iter().map(|(_, n)| n).collect())
    }

    pub fn load(&self, name: &str) -> Result<Canvas, StoreError> {
        let bytes = fs::read(self.path(name)?)?;
        let v: VersionOnly = serde_json::from_slice(&bytes)?;
        if v.version > VERSION {
            return Err(StoreError::TooNew(v.version));
        }
        let canvas: Canvas = serde_json::from_slice(&bytes)?;
        canvas.validate().map_err(StoreError::Invalid)?;
        Ok(canvas)
    }

    /// Writes atomically. Returns false when the bytes on disk already match.
    pub fn save(&self, name: &str, canvas: &Canvas) -> Result<bool, StoreError> {
        let path = self.path(name)?;
        let text = encode(canvas)?;
        if let Ok(old) = fs::read(&path) {
            if old == text.as_bytes() {
                return Ok(false);
            }
            let v: VersionOnly = serde_json::from_slice(&old)?;
            if v.version > VERSION {
                return Err(StoreError::TooNew(v.version));
            }
        }
        fs::create_dir_all(&self.dir)?;
        let tmp = self.dir.join(format!(".{name}{EXT}.tmp"));
        fs::write(&tmp, text)?;
        fs::rename(&tmp, &path)?;
        Ok(true)
    }

    pub fn create(&self, name: &str) -> Result<(), StoreError> {
        if self.exists(name) {
            return Err(StoreError::Exists(name.to_string()));
        }
        self.save(name, &Canvas::default()).map(|_| ())
    }

    pub fn rename(&self, from: &str, to: &str) -> Result<(), StoreError> {
        let (a, b) = (self.path(from)?, self.path(to)?);
        if b.exists() {
            return Err(StoreError::Exists(to.to_string()));
        }
        fs::rename(a, b)?;
        let _ = fs::rename(self.history_path(from)?, self.history_path(to)?);
        Ok(())
    }

    pub fn delete(&self, name: &str) -> Result<(), StoreError> {
        fs::remove_file(self.path(name)?)?;
        let _ = fs::remove_file(self.history_path(name)?);
        Ok(())
    }

    pub fn load_state(&self) -> State {
        fs::read(self.state_dir.join("state.json"))
            .ok()
            .and_then(|b| serde_json::from_slice(&b).ok())
            .unwrap_or_default()
    }

    pub fn save_state(&self, state: &State) -> Result<(), StoreError> {
        fs::create_dir_all(&self.state_dir)?;
        Ok(fs::write(
            self.state_dir.join("state.json"),
            serde_json::to_vec_pretty(state)?,
        )?)
    }

    fn history_path(&self, name: &str) -> Result<PathBuf, StoreError> {
        validate_name(name)?;
        Ok(self.state_dir.join("history").join(format!("{name}.json")))
    }

    /// Hashes the canvas file on disk with FNV-1a, since std hashers change across releases: the saved inverses only fit those exact bytes,
    /// and the UI compares it to spot another process rewriting the file.
    pub fn canvas_hash(&self, name: &str) -> Option<u64> {
        let bytes = fs::read(self.path(name).ok()?).ok()?;
        Some(bytes.iter().fold(0xcbf29ce484222325, |h, &b| {
            (h ^ u64::from(b)).wrapping_mul(0x100000001b3)
        }))
    }

    /// Call after `save`. An empty stack removes the file.
    pub fn save_history(&self, name: &str, undo: Vec<Command>) -> Result<(), StoreError> {
        let path = self.history_path(name)?;
        let Some(canvas_hash) = self.canvas_hash(name).filter(|_| !undo.is_empty()) else {
            let _ = fs::remove_file(path);
            return Ok(());
        };
        fs::create_dir_all(self.state_dir.join("history"))?;
        Ok(fs::write(
            path,
            serde_json::to_vec(&HistoryFile { canvas_hash, undo })?,
        )?)
    }

    /// Empty when the canvas file changed since the history was saved.
    pub fn load_history(&self, name: &str) -> Vec<Command> {
        let file: Option<HistoryFile> = self
            .history_path(name)
            .ok()
            .and_then(|p| fs::read(p).ok())
            .and_then(|b| serde_json::from_slice(&b).ok());
        match file {
            Some(f) if Some(f.canvas_hash) == self.canvas_hash(name) => f.undo,
            _ => vec![],
        }
    }
}
