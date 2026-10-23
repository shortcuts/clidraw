use super::{boxed, Grid};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MenuItem {
    Canvases,
    FitAll,
    ExportSvg,
    ExportMermaid,
    ExportText,
    Quit,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ContextItem {
    EditLabel,
    Duplicate,
    Delete,
    Front,
    Forward,
    Backward,
    Back,
    ResetRoute,
    Paste,
    SelectAll,
    FitAll,
}

/// Right-click menu: a box with its top-left at the pointer, clamped to the screen.
#[derive(Clone, Debug)]
pub struct Context {
    pub at: (i32, i32),
    title: &'static str,
    items: Vec<(ContextItem, &'static str)>,
}

impl Context {
    pub fn for_element(at: (i32, i32), screen: (u16, u16), single: bool, bent: bool) -> Context {
        let edit = single.then_some((ContextItem::EditLabel, "Edit label"));
        let items = edit.into_iter().chain([
            (ContextItem::Duplicate, "Duplicate"),
            (ContextItem::Delete, "Delete"),
            (ContextItem::Front, "Bring to front"),
            (ContextItem::Forward, "Raise"),
            (ContextItem::Backward, "Lower"),
            (ContextItem::Back, "Send to back"),
        ]);
        let reset = bent.then_some((ContextItem::ResetRoute, "Reset route"));
        let items = items.chain(reset);
        Context::new(at, screen, "Element", items.collect())
    }

    pub fn for_canvas(at: (i32, i32), screen: (u16, u16), can_paste: bool) -> Context {
        let paste = can_paste.then_some((ContextItem::Paste, "Paste"));
        let items = paste.into_iter().chain([
            (ContextItem::SelectAll, "Select all"),
            (ContextItem::FitAll, "Fit all"),
        ]);
        Context::new(at, screen, "Canvas", items.collect())
    }

    fn new(
        at: (i32, i32),
        screen: (u16, u16),
        title: &'static str,
        items: Vec<(ContextItem, &'static str)>,
    ) -> Context {
        let (w, h) = Context::size(&items);
        let at = (
            at.0.min(i32::from(screen.0) - w).max(0),
            at.1.min(i32::from(screen.1) - h).max(0),
        );
        Context { at, title, items }
    }

    fn size(items: &[(ContextItem, &str)]) -> (i32, i32) {
        let widest = items.iter().map(|e| e.1.len()).max().unwrap_or(0);
        (widest as i32 + 4, items.len() as i32 + 2)
    }

    fn rect(&self) -> (i32, i32, i32, i32) {
        let (w, h) = Context::size(&self.items);
        (self.at.0, self.at.1, w, h)
    }

    pub fn labels(&self) -> Vec<&'static str> {
        self.items.iter().map(|e| e.1).collect()
    }

    /// None when the click lands outside every entry.
    pub fn entry_at(&self, col: i32, row: i32) -> Option<ContextItem> {
        let (x, y, w, _) = self.rect();
        let i = usize::try_from(row - y - 1).ok()?;
        (col > x && col < x + w - 1).then(|| self.items.get(i).map(|e| e.0))?
    }

    pub fn inside(&self, col: i32, row: i32) -> bool {
        let (x, y, w, h) = self.rect();
        col >= x && col < x + w && row >= y && row < y + h
    }

    pub fn draw(&self, grid: &mut Grid) {
        let r = self.rect();
        boxed(grid, r, self.title);
        for (i, (_, label)) in self.items.iter().enumerate() {
            grid.text(r.0 + 2, r.1 + 1 + i as i32, label, 0);
        }
    }
}
