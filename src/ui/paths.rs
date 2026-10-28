use super::list::{Anchor, List, Row};
use crate::config::expand_home;

pub struct Entry {
    pub name: String,
    pub dir: bool,
}

/// The directory part of `arg`, kept verbatim with its separator, and the name prefix after it.
fn split(arg: &str) -> (&str, &str) {
    let at = arg
        .rfind(['/', std::path::MAIN_SEPARATOR])
        .map_or(0, |i| i + 1);
    arg.split_at(at)
}

/// Entries of the directory named by `arg` whose names start with the rest of `arg`, directories first.
pub fn complete(arg: &str) -> Vec<Entry> {
    let (head, tail) = split(arg);
    let dir = if head.is_empty() {
        ".".into()
    } else {
        expand_home(head)
    };
    // shortcut: rereads the directory on every draw, cache by arg if a huge directory lags
    let Ok(read) = std::fs::read_dir(dir) else {
        return Vec::new();
    };
    let mut found: Vec<Entry> = read
        .flatten()
        .filter_map(|e| {
            let name = e.file_name().to_string_lossy().into_owned();
            let shown = name.starts_with(tail) && (tail.starts_with('.') || !name.starts_with('.'));
            shown.then(|| Entry {
                dir: e.path().is_dir(),
                name,
            })
        })
        .collect();
    found.sort_by(|a, b| b.dir.cmp(&a.dir).then_with(|| a.name.cmp(&b.name)));
    found
}

/// The argument after accepting `entry`: a directory ends in `/` so the next listing descends into it.
pub fn accepted(arg: &str, entry: &Entry) -> String {
    let sep = if entry.dir { "/" } else { "" };
    format!("{}{}{sep}", split(arg).0, entry.name)
}

pub fn drawer(arg: &str, sel: Option<usize>, bar: i32) -> List<usize> {
    let rows = complete(arg)
        .into_iter()
        .enumerate()
        .map(|(value, e)| Row {
            value,
            label: if e.dir {
                format!("{}/", e.name)
            } else {
                e.name
            },
            key: String::new(),
        })
        .collect();
    List::new("Files", rows)
        .headless()
        .min_width(44)
        .max_rows(3)
        .anchor(Anchor::Bottom(bar))
        .select(sel.unwrap_or(0))
}
