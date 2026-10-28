use super::state::Gesture;
use super::{Edit, Session, Tool};
use crate::config::CommandUi;
use crate::model::{Align, Command, Z};
use crate::view::Zoom;
use crossterm::event::{KeyCode, KeyEvent, KeyEventKind, KeyModifiers};

#[derive(Debug, PartialEq, Eq)]
pub enum KeyOutcome {
    None,
    Quit,
    Help,
    Command,
    Find,
    FindNext,
    FindPrev,
    Actions,
    Map,
    Notice(String),
}

fn notice(text: Option<String>) -> KeyOutcome {
    text.map_or(KeyOutcome::None, KeyOutcome::Notice)
}

/// Applies a text-editing key to the label; false leaves the key to the caller.
pub fn edit_key(e: &mut Edit, k: &KeyEvent) -> bool {
    let m = k.modifiers;
    let shift = m.contains(KeyModifiers::SHIFT);
    // macOS Terminal sends Option-arrows as Alt-b and Alt-f.
    match (
        k.code,
        m.contains(KeyModifiers::CONTROL),
        m.contains(KeyModifiers::ALT),
    ) {
        (KeyCode::Char('w'), true, _) => e.delete_word_left(),
        (KeyCode::Char('u'), true, _) => e.delete_to_start(),
        (KeyCode::Char('a'), true, _) => e.select_all(),
        (KeyCode::Left, true, _) | (KeyCode::Left, _, true) | (KeyCode::Char('b'), false, true) => {
            e.word_left(shift)
        }
        (KeyCode::Right, true, _)
        | (KeyCode::Right, _, true)
        | (KeyCode::Char('f'), false, true) => e.word_right(shift),
        (KeyCode::Left, false, false) => e.left(shift),
        (KeyCode::Right, false, false) => e.right(shift),
        (KeyCode::Home, false, false) => e.home(shift),
        (KeyCode::End, false, false) => e.end(shift),
        (KeyCode::Delete, false, false) => e.delete(),
        (KeyCode::Backspace, false, false) => e.backspace(),
        (KeyCode::Enter, _, _) if e.multiline && (shift || m.contains(KeyModifiers::ALT)) => {
            e.insert("\n")
        }
        (KeyCode::Char('j'), true, _) if e.multiline => e.insert("\n"),
        (KeyCode::Char(c), false, false) => e.insert(c.encode_utf8(&mut [0; 4])),
        _ => return false,
    }
    true
}

pub fn handle_key(s: &mut Session, k: KeyEvent) -> KeyOutcome {
    if k.kind == KeyEventKind::Release {
        return KeyOutcome::None;
    }
    if s.edit.as_mut().is_some_and(|e| edit_key(e, &k)) {
        return KeyOutcome::None;
    }
    let ctrl = k.modifiers.contains(KeyModifiers::CONTROL);
    if s.present {
        return match k.code {
            KeyCode::Char('c') if ctrl => KeyOutcome::Quit,
            KeyCode::Esc => {
                s.present = false;
                KeyOutcome::None
            }
            _ => KeyOutcome::None,
        };
    }
    if s.zoomed_out() {
        return match k.code {
            KeyCode::Char('c') if ctrl => KeyOutcome::Quit,
            KeyCode::Char('p') if ctrl && s.idle() && s.command_ui == CommandUi::Palette => {
                KeyOutcome::Actions
            }
            KeyCode::Char('z') if !ctrl => KeyOutcome::Map,
            KeyCode::Esc => {
                s.zoom_to(Zoom::One, s.viewport_center());
                KeyOutcome::None
            }
            KeyCode::Char('?') => KeyOutcome::Help,
            _ => KeyOutcome::None,
        };
    }
    if ctrl {
        match k.code {
            KeyCode::Char('c') => return KeyOutcome::Quit,
            KeyCode::Char('r') => return notice(s.redo()),
            KeyCode::Char('a') => s.select_all(),
            KeyCode::Char('p') if s.idle() && s.command_ui == CommandUi::Palette => {
                return KeyOutcome::Actions
            }
            KeyCode::Char('b') => s.toggle_bold(),
            _ => {}
        }
        return KeyOutcome::None;
    }
    if s.edit.is_some() {
        match k.code {
            KeyCode::Enter => s.commit_edit(),
            KeyCode::Esc => s.edit = None,
            _ => {}
        }
        return KeyOutcome::None;
    }
    let k = match (k.code, s.idle()) {
        (KeyCode::Char(c @ ('h' | 'j' | 'k' | 'l' | 'H' | 'J' | 'K' | 'L')), true) => {
            let code = match c.to_ascii_lowercase() {
                'h' => KeyCode::Left,
                'j' => KeyCode::Down,
                'k' => KeyCode::Up,
                _ => KeyCode::Right,
            };
            let shift = if c.is_ascii_uppercase() {
                KeyModifiers::SHIFT
            } else {
                KeyModifiers::NONE
            };
            KeyEvent::new(code, k.modifiers | shift)
        }
        _ => k,
    };
    match k.code {
        KeyCode::Esc if s.cancel_gesture() => {}
        KeyCode::Esc => {
            s.selection.clear();
            s.tool = Tool::Pointer;
        }
        KeyCode::Char('?') => return KeyOutcome::Help,
        KeyCode::Char(':') if s.command_ui == CommandUi::Vim => return KeyOutcome::Command,
        KeyCode::Char('/') => return KeyOutcome::Find,
        KeyCode::Char('n') => return KeyOutcome::FindNext,
        KeyCode::Char('N') => return KeyOutcome::FindPrev,
        KeyCode::Char(c @ (']' | '[' | '}' | '{'))
            if matches!(s.gesture, Gesture::Idle) && !s.selection.is_empty() =>
        {
            let z = match c {
                ']' => Z::Forward,
                '[' => Z::Backward,
                '}' => Z::Front,
                _ => Z::Back,
            };
            s.reorder(z);
        }
        KeyCode::Char(c @ ('<' | '|' | '>' | '^' | '-' | '_' | '=' | '#'))
            if matches!(s.gesture, Gesture::Idle) && !s.selection.is_empty() =>
        {
            let a = match c {
                '<' => Align::Left,
                '|' => Align::Center,
                '>' => Align::Right,
                '^' => Align::Top,
                '-' => Align::Middle,
                '_' => Align::Bottom,
                '=' => Align::DistributeH,
                _ => Align::DistributeV,
            };
            s.align(a);
        }
        KeyCode::Char(c @ ('g' | 'G'))
            if matches!(s.gesture, Gesture::Idle) && !s.selection.is_empty() =>
        {
            s.group(c == 'g');
        }
        KeyCode::Char('y') => s.copy(),
        KeyCode::Char('x') => s.cut(),
        KeyCode::Char('p') => s.paste(),
        KeyCode::Char('u') => return notice(s.undo()),
        KeyCode::Char('z') if matches!(s.gesture, Gesture::Idle) => return KeyOutcome::Map,
        KeyCode::Char(c) if matches!(s.gesture, Gesture::Idle) => {
            if let Some(t) = Tool::ALL.into_iter().find(|t| t.key() == Some(c)) {
                s.tool = t;
            }
        }
        KeyCode::Left | KeyCode::Right | KeyCode::Up | KeyCode::Down
            if k.modifiers.contains(KeyModifiers::SHIFT) && matches!(s.gesture, Gesture::Idle) =>
        {
            s.connect(match k.code {
                KeyCode::Left => (-1, 0),
                KeyCode::Right => (1, 0),
                KeyCode::Up => (0, -1),
                _ => (0, 1),
            });
        }
        KeyCode::Left | KeyCode::Right | KeyCode::Up | KeyCode::Down
            if matches!(s.gesture, Gesture::Idle) =>
        {
            let (dx, dy) = match k.code {
                KeyCode::Left => (-1, 0),
                KeyCode::Right => (1, 0),
                KeyCode::Up => (0, -1),
                _ => (0, 1),
            };
            if s.selection.is_empty() {
                s.glide = None;
                s.view.pan(s.view.world_step(dx), s.view.world_step(dy));
                return KeyOutcome::None;
            }
            let cmds = s
                .selection
                .iter()
                .cloned()
                .map(|id| Command::Move { id, dx, dy })
                .collect();
            s.exec(Command::Batch(cmds));
        }
        KeyCode::Delete | KeyCode::Backspace if !s.selection.is_empty() => s.delete_selection(),
        _ => {}
    }
    KeyOutcome::None
}

pub fn handle_paste(s: &mut Session, text: &str) {
    if let Some(edit) = &mut s.edit {
        let keep = |c: &char| !c.is_control() || (edit.multiline && *c == '\n');
        let text = text.chars().filter(keep).collect::<String>();
        edit.insert(&text);
    }
}
