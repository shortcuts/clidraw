use super::App;
use crate::export;
use crate::ui::prompt::{Prompt, PromptKind};
use crate::ui::scope::{ExportFormat, ScopeItem};
use crate::ui::Overlay;
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers, MouseButton, MouseEvent, MouseEventKind};

type Area = (i32, i32, i32, i32);

/// An area export waiting for the user to drag its rectangle.
pub struct Pick {
    format: ExportFormat,
    start: Option<(i32, i32)>,
}

impl App {
    pub(super) fn scope_item(&mut self, item: ScopeItem, format: ExportFormat) {
        self.overlay = Overlay::None;
        match item {
            ScopeItem::Whole => self.open_export_prompt(format, None),
            ScopeItem::Area => {
                self.pick = Some(Pick {
                    format,
                    start: None,
                })
            }
        }
    }

    /// True when the pick consumed the event.
    pub(super) fn pick_mouse(&mut self, m: MouseEvent) -> bool {
        let Some(pick) = &mut self.pick else {
            return false;
        };
        let at = self
            .session
            .view
            .to_world(i32::from(m.column), i32::from(m.row));
        match m.kind {
            MouseEventKind::Down(MouseButton::Left) => pick.start = Some(at),
            MouseEventKind::Drag(MouseButton::Left) => {
                self.session.band = pick.start.map(|s| (s, at));
            }
            MouseEventKind::Up(MouseButton::Left) => {
                // The Up of the click that chose "Area" arrives before any Down.
                let Some(start) = pick.start else {
                    return true;
                };
                let format = pick.format;
                self.pick = None;
                self.session.band = None;
                self.finish_pick(format, start, at);
            }
            _ => return false,
        }
        true
    }

    /// True when the pick consumed the key; Ctrl-C still quits.
    pub(super) fn pick_key(&mut self, k: KeyEvent) -> bool {
        if self.pick.is_none() || k.modifiers.contains(KeyModifiers::CONTROL) {
            return false;
        }
        if k.code == KeyCode::Esc {
            self.pick = None;
            self.session.band = None;
        }
        true
    }

    fn finish_pick(&mut self, format: ExportFormat, a: (i32, i32), b: (i32, i32)) {
        let area = (
            a.0.min(b.0),
            a.1.min(b.1),
            (a.0 - b.0).abs() + 1,
            (a.1 - b.1).abs() + 1,
        );
        if self.session.canvas.within(area).elements.is_empty() {
            self.status = Some("nothing inside the area".into());
        } else {
            self.open_export_prompt(format, Some(area));
        }
    }

    fn open_export_prompt(&mut self, format: ExportFormat, area: Option<Area>) {
        let (kind, ext) = match format {
            ExportFormat::Svg => (PromptKind::ExportSvg(area), "svg"),
            ExportFormat::Mermaid => (PromptKind::ExportMermaid(area), "mmd"),
        };
        self.overlay = Overlay::Prompt(Prompt::new(kind, format!("{}.{ext}", self.name)));
    }

    pub(super) fn export(&mut self, path: &str, format: ExportFormat, area: Option<Area>) {
        let canvas = match area {
            Some(a) => self.session.canvas.within(a),
            None => self.session.canvas.clone(),
        };
        let (text, skipped) = match format {
            ExportFormat::Svg => (export::svg(&canvas), 0),
            ExportFormat::Mermaid => {
                let out = export::mermaid(&canvas);
                (out.text, out.skipped)
            }
        };
        self.status = Some(match std::fs::write(path, text) {
            Ok(()) if skipped > 0 => {
                format!("exported {path}, skipped {skipped} element(s) with no Mermaid form")
            }
            Ok(()) => format!("exported {path}"),
            Err(e) => format!("export failed: {e}"),
        });
    }
}
