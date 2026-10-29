use super::App;
use crate::config::expand_home;
use crate::export;
use crate::model::Canvas;
use crate::ui::prompt::{Extent, Prompt, PromptKind};
use crate::ui::scope::{self, ExportFormat, ScopeItem, Sink};
use crate::ui::{Overlay, Status};
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers, MouseButton, MouseEvent, MouseEventKind};

/// An area export waiting for the user to drag its rectangle.
pub struct Pick {
    sink: Sink,
    start: Option<(i32, i32)>,
}

impl App {
    pub(super) fn pick_scope(&mut self, sink: Sink) {
        self.overlay = Overlay::Scope(sink, scope::list(sink));
    }

    pub(super) fn scope_item(&mut self, item: ScopeItem, sink: Sink) {
        self.overlay = Overlay::None;
        match item {
            ScopeItem::Whole => self.open_export_prompt(sink, Extent::Whole),
            ScopeItem::Selection if self.session.selection.is_empty() => {
                self.status = Some(Status::error("Nothing selected: select a shape first"))
            }
            ScopeItem::Selection => self.open_export_prompt(sink, Extent::Selection),
            ScopeItem::Area => self.pick = Some(Pick { sink, start: None }),
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
            MouseEventKind::Down(MouseButton::Left) => {
                if self
                    .session
                    .in_viewport(i32::from(m.column), i32::from(m.row))
                {
                    pick.start = Some(at);
                }
            }
            MouseEventKind::Drag(MouseButton::Left) => {
                self.session.band = pick.start.map(|s| (s, at));
            }
            MouseEventKind::Up(MouseButton::Left) => {
                // The Up of the click that chose "Area" arrives before any Down.
                let Some(start) = pick.start else {
                    return true;
                };
                let sink = pick.sink;
                self.pick = None;
                self.session.band = None;
                self.finish_pick(sink, start, at);
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

    fn finish_pick(&mut self, sink: Sink, a: (i32, i32), b: (i32, i32)) {
        let area = (
            a.0.min(b.0),
            a.1.min(b.1),
            (a.0 - b.0).abs() + 1,
            (a.1 - b.1).abs() + 1,
        );
        if self.session.canvas.within(area).elements.is_empty() {
            self.status = Some(Status::error("Nothing inside the area: drag over a shape"));
        } else {
            self.open_export_prompt(sink, Extent::Area(area));
        }
    }

    fn open_export_prompt(&mut self, sink: Sink, extent: Extent) {
        let (kind, ext) = match sink {
            Sink::File(ExportFormat::Svg) => (PromptKind::ExportSvg(extent), "svg"),
            Sink::File(ExportFormat::Mermaid) => (PromptKind::ExportMermaid(extent), "mmd"),
            Sink::File(ExportFormat::Clidraw) => {
                (PromptKind::ExportClidraw(extent), "clidraw.json")
            }
            Sink::ShareLink => return self.copy_share(extent),
            Sink::ObsidianCopy => return self.copy_obsidian(extent),
            Sink::ObsidianNote => return self.open_notes(extent),
        };
        self.overlay = Overlay::Prompt(Prompt::new(kind, format!("{}.{ext}", self.name)));
    }

    pub(super) fn export(&mut self, path: &str, format: ExportFormat, extent: Extent) {
        let path = expand_home(path);
        if std::path::Path::new(&path).exists() {
            let kind = PromptKind::ConfirmOverwrite(path, format, extent);
            self.overlay = Overlay::Prompt(Prompt::new(kind, String::new()));
        } else {
            self.write_export(&path, format, extent);
        }
    }

    /// Copies the whole canvas as clidraw JSON, for an issue report or a prompt.
    pub(super) fn copy_clidraw(&mut self) {
        let (text, _) = export::render(
            &self.session.canvas,
            ExportFormat::Clidraw,
            self.export_padding,
        );
        self.copy = Some(text);
        self.status = Some(Status::notice("Copied clidraw JSON to the clipboard"));
    }

    fn extent_canvas(&self, extent: Extent) -> Canvas {
        let mut canvas = match extent {
            Extent::Whole => self.session.canvas.clone(),
            Extent::Area(a) => self.session.canvas.within(a),
            Extent::Selection => self
                .session
                .canvas
                .clone_canvas(&self.session.selection, 0, 0),
        };
        canvas.dir = self.session.canvas.dir;
        canvas
    }

    fn copy_share(&mut self, extent: Extent) {
        let canvas = self.extent_canvas(extent);
        let (text, skipped) = export::render(&canvas, ExportFormat::Mermaid, self.export_padding);
        // shortcut: OSC 52 length cap unmeasured, upgrade on truncated-copy reports
        let url = format!("{SHARE_URL}{}", base64(text.as_bytes(), URL_SAFE, false));
        self.copy = Some(url.clone());
        self.status = Some(match skipped {
            0 => Status::notice(format!("Copied share link: {url}")),
            n => Status::notice(format!(
                "Copied share link, skipped {n} element(s) with no Mermaid form: {url}"
            )),
        });
    }

    /// The fenced Mermaid block of the extent and the count of elements it skips.
    pub(super) fn obsidian_block(&self, extent: Extent) -> (String, usize) {
        let canvas = self.extent_canvas(extent);
        let (text, skipped) = export::render(&canvas, ExportFormat::Mermaid, self.export_padding);
        let longest_run = text.split(|c| c != '`').map(str::len).max().unwrap_or(0);
        let fence = "`".repeat(longest_run.max(2) + 1);
        (
            format!("{fence}mermaid\n{}\n{fence}\n", text.trim_end()),
            skipped,
        )
    }

    fn copy_obsidian(&mut self, extent: Extent) {
        let (block, skipped) = self.obsidian_block(extent);
        self.copy = Some(block);
        self.status = Some(match skipped {
            0 => Status::notice("Copied for Obsidian"),
            n => Status::notice(format!(
                "Copied for Obsidian, skipped {n} element(s) with no Mermaid form"
            )),
        });
    }

    pub(super) fn write_export(&mut self, path: &str, format: ExportFormat, extent: Extent) {
        let canvas = self.extent_canvas(extent);
        let (text, skipped) = export::render(&canvas, format, self.export_padding);
        self.status = Some(match std::fs::write(path, text) {
            Ok(()) if skipped > 0 => Status::notice(format!(
                "Exported {path}, skipped {skipped} element(s) with no Mermaid form"
            )),
            Ok(()) => Status::notice(format!("Exported {path}")),
            Err(e) => Status::error(format!("Export failed: {e}")),
        });
    }
}

const STANDARD: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
const URL_SAFE: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789-_";
const SHARE_URL: &str = "https://clidraw.shrtcts.fr/view.html#";

fn base64(bytes: &[u8], alphabet: &[u8; 64], pad: bool) -> String {
    let mut out = String::new();
    for c in bytes.chunks(3) {
        let n = c.iter().fold(0u32, |n, b| n << 8 | u32::from(*b)) << (8 * (3 - c.len()));
        for i in 0..4 {
            if i <= c.len() {
                out.push(alphabet[(n >> (18 - 6 * i) & 63) as usize] as char);
            } else if pad {
                out.push('=');
            }
        }
    }
    out
}

/// Terminals that ignore OSC 52 drop it silently, so the copy then does nothing.
pub(super) fn osc52(text: &str) -> String {
    format!(
        "\x1b]52;c;{}\x1b\\",
        base64(text.as_bytes(), STANDARD, true)
    )
}
