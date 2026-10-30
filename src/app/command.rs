use super::App;
use crate::config::CommandUi;
use crate::input::edit_key;
use crate::model::Dir;
use crate::ui::paths;
use crate::ui::prompt::{self, Prompt, PromptKind};
use crate::ui::registry::{self, Binding};
use crate::ui::scope::{self, ExportFormat, Sink};
use crate::ui::{Overlay, Perf, Status};
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

impl App {
    pub(super) fn prompt_key(&mut self, k: KeyEvent) {
        let Overlay::Prompt(p) = &mut self.overlay else {
            return;
        };
        if p.is_confirm() {
            match k.code {
                KeyCode::Left | KeyCode::Char('h') => return p.cancel_focused = false,
                KeyCode::Right | KeyCode::Char('l') => return p.cancel_focused = true,
                _ => {}
            }
        }
        if p.kind == PromptKind::ResetSettings && k.code == KeyCode::Char(' ') {
            return p.checked = !p.checked;
        }
        if edit_key(&mut p.edit, &k) {
            p.sel = 0;
            p.path_sel = None;
            return;
        }
        let ctrl = k.modifiers.contains(KeyModifiers::CONTROL);
        let files = p.path_arg().map_or(0, |a| paths::complete(a).len());
        if files > 0 {
            let step = match k.code {
                KeyCode::Up => Some(-1),
                KeyCode::Down => Some(1),
                KeyCode::Char('p') if ctrl => Some(-1),
                KeyCode::Char('n') if ctrl => Some(1),
                _ => None,
            };
            if let Some(step) = step {
                let at = p
                    .path_sel
                    .map_or(0, |i| i.saturating_add_signed(step).min(files - 1));
                return p.path_sel = Some(at);
            }
            match (k.code, p.path_sel) {
                (KeyCode::Tab, i) => return p.accept_path(i.unwrap_or(0)),
                (KeyCode::Enter, Some(i)) => return p.accept_path(i),
                _ => {}
            }
        }
        let by = match k.code {
            KeyCode::Up => -1,
            KeyCode::Down => 1,
            KeyCode::Char('p') if ctrl => -1,
            KeyCode::Char('n') if ctrl => 1,
            KeyCode::Esc => return self.overlay = Overlay::None,
            KeyCode::Enter if p.cancel_focused => return self.overlay = Overlay::None,
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

    /// `key=value` sets, `key=` removes, and no argument reads back the one selected element.
    fn meta(&mut self, arg: &str) {
        let usage = || Some(Status::error("Usage: :meta <key>=<value>"));
        self.status = if arg.is_empty() {
            match self.session.selection.as_slice() {
                [id] => {
                    let meta = self.session.canvas.get(id).map(|e| e.meta().clone());
                    let pairs: Vec<String> = meta
                        .iter()
                        .flatten()
                        .map(|(k, v)| format!("{k}={v}"))
                        .collect();
                    Some(Status::notice(if pairs.is_empty() {
                        "No metadata".to_string()
                    } else {
                        pairs.join(", ")
                    }))
                }
                _ => usage(),
            }
        } else {
            match arg.split_once('=') {
                Some((k, v)) if self.session.set_meta(k.trim(), v.trim()) => None,
                _ => usage(),
            }
        };
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
                "svg" => self.pick_scope(Sink::File(ExportFormat::Svg)),
                "mmd" => self.pick_scope(Sink::File(ExportFormat::Mermaid)),
                "clidraw" => self.pick_scope(Sink::File(ExportFormat::Clidraw)),
                _ => self.status = Some(Status::error("Usage: :export [svg|mmd|clidraw]")),
            },
            "link" if !self.session.set_link(arg.trim()) => {
                self.status = Some(Status::error("Usage: :link <http(s) or mailto url>"))
            }
            "link" => {}
            "meta" => self.meta(arg.trim()),
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
                self.status = Some(Status::error(match self.session.command_ui {
                    CommandUi::Palette => format!(
                        "Unknown command: {cmd} ({} lists commands)",
                        registry::by_id("palette").first_label()
                    ),
                    CommandUi::Vim => format!("Unknown command: {cmd}"),
                }))
            }
        }
    }
}
