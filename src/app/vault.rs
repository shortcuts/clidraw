use super::App;
use crate::store::validate_name;
use crate::ui::notes::{NoteHit, Notes};
use crate::ui::prompt::{Extent, Prompt, PromptKind};
use crate::ui::{Overlay, Status};
use std::io::{self, Write};
use std::path::Path;

impl App {
    pub(super) fn open_notes(&mut self, extent: Extent) {
        let Some(vault) = &self.obsidian_vault else {
            return;
        };
        match scan(vault) {
            Ok(paths) => self.overlay = Overlay::Notes(extent, Notes::new(paths)),
            Err(e) => self.status = Some(Status::error(format!("Cannot read vault: {e}"))),
        }
    }

    pub(super) fn note_hit(&mut self, hit: NoteHit, extent: Extent) {
        let Overlay::Notes(_, notes) = &self.overlay else {
            return;
        };
        match hit {
            NoteHit::New => {
                let kind = PromptKind::NewNote(extent);
                self.overlay = Overlay::Prompt(Prompt::new(kind, self.name.clone()));
            }
            NoteHit::Open(i) => {
                let name = notes.paths[i].clone();
                self.overlay = Overlay::None;
                self.append_note(&name, extent);
            }
        }
    }

    pub(super) fn new_note(&mut self, typed: &str, extent: Extent) {
        let mut name = typed.to_string();
        if !name.to_lowercase().ends_with(".md") {
            name.push_str(".md");
        }
        if let Err(e) = validate_name(&name) {
            self.status = Some(Status::error(e.to_string()));
            return;
        }
        let Some(vault) = &self.obsidian_vault else {
            return;
        };
        let (block, skipped) = self.obsidian_block(extent);
        let made = std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(vault.join(&name))
            .and_then(|mut f| f.write_all(block.as_bytes()));
        match made {
            Err(e) if e.kind() == io::ErrorKind::AlreadyExists => {
                let kind = PromptKind::ConfirmAppend(name, extent);
                self.overlay = Overlay::Prompt(Prompt::new(kind, String::new()));
            }
            made => self.saved(&name, skipped, made),
        }
    }

    pub(super) fn append_note(&mut self, name: &str, extent: Extent) {
        let Some(vault) = &self.obsidian_vault else {
            return;
        };
        let path = vault.join(name);
        let (block, skipped) = self.obsidian_block(extent);
        let appended = std::fs::read_to_string(&path).and_then(|existing| {
            let text = format!("{}{block}", separator(&existing));
            std::fs::OpenOptions::new()
                .append(true)
                .open(&path)?
                .write_all(text.as_bytes())
        });
        self.saved(name, skipped, appended);
    }

    fn saved(&mut self, name: &str, skipped: usize, result: io::Result<()>) {
        self.status = Some(match result {
            Ok(()) if skipped > 0 => Status::notice(format!(
                "Saved to {name}, skipped {skipped} element(s) with no Mermaid form"
            )),
            Ok(()) => Status::notice(format!("Saved to {name}")),
            Err(e) => Status::error(format!("Cannot write {name}: {e}")),
        });
    }
}

/// Text that puts a blank line between the note and the appended block.
fn separator(existing: &str) -> &'static str {
    if existing.is_empty() || existing.ends_with("\n\n") {
        ""
    } else if existing.ends_with('\n') {
        "\n"
    } else {
        "\n\n"
    }
}

/// Sorted `/`-separated paths of the vault's `.md` files, hidden folders skipped.
// shortcut: no cap on note count, add one if a vault of tens of thousands of notes opens slowly
fn scan(vault: &Path) -> io::Result<Vec<String>> {
    let mut found = vec![];
    walk(vault, "", &mut found, true)?;
    found.sort();
    Ok(found)
}

fn walk(dir: &Path, prefix: &str, found: &mut Vec<String>, root: bool) -> io::Result<()> {
    let entries = match std::fs::read_dir(dir) {
        Ok(e) => e,
        Err(e) if root => return Err(e),
        Err(_) => return Ok(()),
    };
    for entry in entries.flatten() {
        let name = entry.file_name().to_string_lossy().into_owned();
        let Ok(kind) = entry.file_type() else {
            continue;
        };
        if name.starts_with('.') {
            continue;
        }
        let rel = format!("{prefix}{name}");
        if kind.is_dir() {
            walk(&entry.path(), &format!("{rel}/"), found, false)?;
        } else if kind.is_file() && name.to_lowercase().ends_with(".md") {
            found.push(rel);
        }
    }
    Ok(())
}
