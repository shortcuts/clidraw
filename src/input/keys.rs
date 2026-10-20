use super::state::Gesture;
use super::{Session, Tool};
use crate::model::Command;
use crossterm::event::{KeyCode, KeyEvent, KeyEventKind, KeyModifiers};

#[derive(Debug, PartialEq, Eq)]
pub enum KeyOutcome {
    None,
    Quit,
    Menu,
    Help,
    Command,
}

pub fn handle_key(s: &mut Session, k: KeyEvent) -> KeyOutcome {
    if k.kind == KeyEventKind::Release {
        return KeyOutcome::None;
    }
    let ctrl = k.modifiers.contains(KeyModifiers::CONTROL);
    if ctrl {
        match k.code {
            KeyCode::Char('c') => return KeyOutcome::Quit,
            KeyCode::Char('z') => s.undo(),
            KeyCode::Char('y') => s.redo(),
            _ => {}
        }
        return KeyOutcome::None;
    }
    if let Some(edit) = &mut s.edit {
        match k.code {
            KeyCode::Char(c) => edit.buf.push(c),
            KeyCode::Backspace => {
                edit.buf.pop();
            }
            KeyCode::Enter => s.commit_edit(),
            KeyCode::Esc => s.edit = None,
            _ => {}
        }
        return KeyOutcome::None;
    }
    match k.code {
        KeyCode::Esc if s.cancel_gesture() => {}
        KeyCode::Esc => return KeyOutcome::Menu,
        KeyCode::Char('?') => return KeyOutcome::Help,
        KeyCode::Char(':') => return KeyOutcome::Command,
        KeyCode::Char(c) if matches!(s.gesture, Gesture::Idle) => {
            if let Some(t) = Tool::ALL.into_iter().find(|t| t.key() == c) {
                s.tool = t;
            }
        }
        KeyCode::Delete | KeyCode::Backspace if !s.selection.is_empty() => {
            let cmds = s.selection.iter().cloned().map(Command::Remove).collect();
            s.exec(Command::Batch(cmds));
        }
        _ => {}
    }
    KeyOutcome::None
}

pub fn handle_paste(s: &mut Session, text: &str) {
    if let Some(edit) = &mut s.edit {
        edit.buf.extend(text.chars().filter(|c| !c.is_control()));
    }
}
