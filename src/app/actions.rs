use super::App;
use crate::ui::actions::Action;
use crate::ui::prompt::{Prompt, PromptKind};
use crate::ui::Overlay;
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

impl App {
    pub(super) fn actions_key(&mut self, k: KeyEvent) {
        let Overlay::Actions(a) = &mut self.overlay else {
            return;
        };
        match k.code {
            KeyCode::Esc => self.overlay = Overlay::None,
            KeyCode::Enter => {
                if let Some(action) = a.selected_action() {
                    self.run_action(action);
                }
            }
            KeyCode::Up => a.up(),
            KeyCode::Down => a.down(),
            KeyCode::Char('p') if k.modifiers.contains(KeyModifiers::CONTROL) => a.up(),
            KeyCode::Char('n') if k.modifiers.contains(KeyModifiers::CONTROL) => a.down(),
            KeyCode::Backspace => a.pop(),
            KeyCode::Char(c) if !k.modifiers.contains(KeyModifiers::CONTROL) => a.push(c),
            _ => {}
        }
    }

    pub(super) fn run_action(&mut self, action: Action) {
        self.overlay = Overlay::None;
        let s = &mut self.session;
        match action {
            Action::Tool(t) => s.tool = t,
            Action::Menu(i) => self.menu_item(i),
            Action::Context(i) => self.context_item(i),
            Action::Align(a) => s.align(a),
            Action::Order(z) => s.reorder(z),
            Action::Group => s.group(true),
            Action::Ungroup => s.group(false),
            Action::Bold => s.toggle_bold(),
            Action::Copy => s.copy(),
            Action::Cut => s.cut(),
            Action::ZoomIn | Action::ZoomOut => {
                s.zoom(
                    if action == Action::ZoomIn { 1 } else { -1 },
                    s.viewport_center(),
                );
            }
            Action::Present => {
                s.selection.clear();
                s.present = true;
            }
            Action::Help => self.overlay = Overlay::Help(0, String::new()),
            Action::Map => self.overlay = Overlay::Map,
            Action::Find => {
                self.overlay = Overlay::Prompt(Prompt::new(PromptKind::Command, "/".into()))
            }
            Action::Ex(line) => self.run_command(line),
        }
    }
}
