use super::App;
use crate::names;
use crate::ui::actions::Action;
use crate::ui::colors;
use crate::ui::prompt::{Prompt, PromptKind};
use crate::ui::Overlay;

impl App {
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
            Action::Style(c) => s.set_choice(c),
            Action::Pick(c) => self.overlay = Overlay::Colors(c, colors::list(c)),
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
            Action::Prefill(text) => {
                self.overlay = Overlay::Prompt(Prompt::new(PromptKind::Command, text.into()))
            }
            Action::NewCanvas => {
                let fresh = names::generate(|n| self.store.exists(n));
                self.overlay = Overlay::Prompt(Prompt::new(PromptKind::NewCanvas, fresh))
            }
            Action::RenameCanvas => {
                let name = self.name.clone();
                self.overlay = Overlay::Prompt(Prompt::new(PromptKind::Rename(name.clone()), name))
            }
            Action::DeleteCanvas => {
                let kind = PromptKind::ConfirmDelete(self.name.clone());
                self.overlay = Overlay::Prompt(Prompt::new(kind, String::new()))
            }
        }
    }
}
