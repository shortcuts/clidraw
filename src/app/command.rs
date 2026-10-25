use super::App;
use crate::input::edit_key;
use crate::model::Dir;
use crate::ui::prompt::{self, Prompt, PromptKind};
use crate::ui::registry::{self, Binding};
use crate::ui::scope::{self, ExportFormat};
use crate::ui::{Overlay, Perf, Status};
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

impl App {
    pub(super) fn prompt_key(&mut self, k: KeyEvent) {
        let Overlay::Prompt(p) = &mut self.overlay else {
            return;
        };
        if edit_key(&mut p.edit, &k) {
            p.sel = 0;
            return;
        }
        let ctrl = k.modifiers.contains(KeyModifiers::CONTROL);
        let by = match k.code {
            KeyCode::Up => -1,
            KeyCode::Down => 1,
            KeyCode::Char('p') if ctrl => -1,
            KeyCode::Char('n') if ctrl => 1,
            KeyCode::Esc => return self.overlay = Overlay::None,
            KeyCode::Enter => return self.confirm(),
            KeyCode::Tab if p.fixes => return p.apply_fix(),
            KeyCode::Tab if p.completing() => {
                if let Some(b) = prompt::command_list(p, 0).selected() {
                    self.complete(b);
                }
                return;
            }
            _ => return,
        };
        if p.completing() {
            let last = prompt::command_list(p, 0).labels().len().saturating_sub(1);
            p.sel = p.sel.saturating_add_signed(by).min(last);
        }
    }

    pub(super) fn complete(&mut self, b: &'static Binding) {
        if let Overlay::Prompt(p) = &mut self.overlay {
            *p = Prompt::new(p.kind.clone(), registry::fill(b));
        }
    }

    pub(super) fn run_command(&mut self, line: &str) {
        let line = line.trim().trim_start_matches(':');
        if let Some(term) = line.strip_prefix('/') {
            if !self.session.find(term.trim()) {
                self.status = Some(Status::error("No match: try another search term"));
            }
            return;
        }
        let (cmd, arg) = line.split_once(char::is_whitespace).unwrap_or((line, ""));
        match cmd {
            "" => {}
            "q" => self.quit = true,
            "w" => self.save(),
            "wq" => {
                self.save();
                self.quit = true;
            }
            "e" if arg.trim().is_empty() => self.status = Some(Status::error("Usage: :e <name>")),
            "e" => self.switch_to(arg.trim()),
            "import" if arg.trim().is_empty() => {
                self.status = Some(Status::error("Usage: :import <path>"))
            }
            "import" => self.import(arg.trim()),
            "mermaid" => {
                let kind = PromptKind::Mermaid(self.mermaid_header.clone());
                let mut p = Prompt::new(kind, String::new());
                p.fixes = self.quick_fixes;
                self.overlay = Overlay::Prompt(p);
            }
            "export" => match arg.trim() {
                "" => self.overlay = Overlay::Formats(scope::formats()),
                "svg" => self.pick_scope(ExportFormat::Svg),
                "mmd" => self.pick_scope(ExportFormat::Mermaid),
                "clidraw" => self.pick_scope(ExportFormat::Clidraw),
                _ => self.status = Some(Status::error("Usage: :export [svg|mmd|clidraw]")),
            },
            "link" if !self.session.set_link(arg.trim()) => {
                self.status = Some(Status::error("Usage: :link <http(s) or mailto url>"))
            }
            "link" => {}
            "lib" => self.library(arg.trim()),
            "perf" => self.perf = self.perf.is_none().then(Perf::default),
            "layout" => match arg.trim() {
                "" | "td" => self.session.layout(Dir::TopDown),
                "lr" => self.session.layout(Dir::LeftRight),
                _ => self.status = Some(Status::error("Usage: :layout [td|lr]")),
            },
            "u" => self.status = self.session.undo().map(Status::notice),
            "redo" => self.status = self.session.redo().map(Status::notice),
            _ => {
                self.status = Some(Status::error(format!(
                    "Unknown command: {cmd} ({} lists commands)",
                    registry::by_id("palette").first_label()
                )))
            }
        }
    }
}
