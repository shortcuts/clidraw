use super::state::Gesture;
use super::{Edit, Session, Tool};
use crate::model::{Align, Command, Z};
use crossterm::event::{KeyCode, KeyEvent, KeyEventKind, KeyModifiers};

#[derive(Debug, PartialEq, Eq)]
pub enum KeyOutcome {
    None,
    Quit,
    Menu,
    Help,
    Command,
    Find,
    Notice(String),
}

fn notice(text: Option<String>) -> KeyOutcome {
    text.map_or(KeyOutcome::None, KeyOutcome::Notice)
}

/// Applies a text-editing key to the label; false leaves the key to the caller.
fn edit_key(e: &mut Edit, k: &KeyEvent) -> bool {
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
    if s.overview {
        return match k.code {
            KeyCode::Char('c') if ctrl => KeyOutcome::Quit,
            KeyCode::Char('z') | KeyCode::Esc if !ctrl => {
                s.overview = false;
                KeyOutcome::None
            }
            KeyCode::Char('?') => KeyOutcome::Help,
            _ => KeyOutcome::None,
        };
    }
    if ctrl {
        match k.code {
            KeyCode::Char('c') => return KeyOutcome::Quit,
            KeyCode::Char('z') => return notice(s.undo()),
            KeyCode::Char('y') => return notice(s.redo()),
            KeyCode::Char('a') => {
                s.selection = s.canvas.elements.iter().map(|e| e.id().clone()).collect()
            }
            KeyCode::Char('f') => return KeyOutcome::Find,
            KeyCode::Char('x') => s.cut(),
            KeyCode::Char('v') => s.paste(),
            KeyCode::Char('d') => s.duplicate(),
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
    match k.code {
        KeyCode::Esc if s.cancel_gesture() => {}
        KeyCode::Esc => return KeyOutcome::Menu,
        KeyCode::Char('?') => return KeyOutcome::Help,
        KeyCode::Char(':') => return KeyOutcome::Command,
        KeyCode::Char(c @ (']' | '[' | '}' | '{'))
            if matches!(s.gesture, Gesture::Idle) && !s.selection.is_empty() =>
        {
            let z = match c {
                ']' => Z::Forward,
                '[' => Z::Backward,
                '}' => Z::Front,
                _ => Z::Back,
            };
            let cmds = s.canvas.z_order(&s.selection, z);
            if !cmds.is_empty() {
                s.exec(Command::Batch(cmds));
            }
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
            let cmds = s.canvas.align(&s.selection, a);
            if !cmds.is_empty() {
                s.exec(Command::Batch(cmds));
            }
        }
        KeyCode::Char(c @ ('g' | 'G'))
            if matches!(s.gesture, Gesture::Idle) && !s.selection.is_empty() =>
        {
            let cmd = if c == 'g' {
                s.canvas.group_cmd(&s.selection)
            } else {
                s.canvas.ungroup_cmd(&s.selection)
            };
            if let Some(cmd) = cmd {
                s.exec(cmd);
            }
        }
        KeyCode::Char('c') => s.copy(),
        KeyCode::Char('z') if matches!(s.gesture, Gesture::Idle) => s.overview = true,
        KeyCode::Char(c) if matches!(s.gesture, Gesture::Idle) => {
            if let Some(t) = Tool::ALL.into_iter().find(|t| t.key() == c) {
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
            if matches!(s.gesture, Gesture::Idle) && !s.selection.is_empty() =>
        {
            let (dx, dy) = match k.code {
                KeyCode::Left => (-1, 0),
                KeyCode::Right => (1, 0),
                KeyCode::Up => (0, -1),
                _ => (0, 1),
            };
            let cmds = s
                .selection
                .iter()
                .cloned()
                .map(|id| Command::Move { id, dx, dy })
                .collect();
            s.exec(Command::Batch(cmds));
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
        let keep = |c: &char| !c.is_control() || (edit.multiline && *c == '\n');
        let text = text.chars().filter(keep).collect::<String>();
        edit.insert(&text);
    }
}
