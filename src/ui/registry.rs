use super::actions::Action;
use super::menu::{ContextItem, MenuItem};
use super::palette::Choice;
use crate::input::Tool;
use crate::model::{Align, Border, LineKind, Stroke, TextAlign, Z};
use crossterm::event::{KeyCode, KeyModifiers};

pub const ZOOM_WHEEL: &str = "Ctrl-wheel";

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Scope {
    Canvas,
    Selection,
    TextEdit,
    Overlay,
    ZoomedOut,
    Tool,
    Command,
    /// Palette-only entries: no help row.
    Palette,
}

#[derive(Clone, Copy, Debug)]
pub struct Key {
    pub code: KeyCode,
    pub mods: KeyModifiers,
}

const fn plain(code: KeyCode) -> Key {
    Key {
        code,
        mods: KeyModifiers::NONE,
    }
}

const fn c(ch: char) -> Key {
    plain(KeyCode::Char(ch))
}

const fn ctrl(ch: char) -> Key {
    Key {
        code: KeyCode::Char(ch),
        mods: KeyModifiers::CONTROL,
    }
}

const fn with(code: KeyCode, mods: KeyModifiers) -> Key {
    Key { code, mods }
}

impl Key {
    /// The only place that renders key text.
    pub fn label(&self) -> String {
        let mut s = String::new();
        for (m, prefix) in [
            (KeyModifiers::CONTROL, "Ctrl-"),
            (KeyModifiers::ALT, "Alt-"),
            (KeyModifiers::SHIFT, "Shift-"),
        ] {
            if self.mods.contains(m) {
                s += prefix;
            }
        }
        s += &match self.code {
            KeyCode::Esc => "Esc".into(),
            KeyCode::Enter => "Enter".into(),
            KeyCode::Delete => "Del".into(),
            KeyCode::Backspace => "Backspace".into(),
            KeyCode::Up => "Up".into(),
            KeyCode::Down => "Down".into(),
            KeyCode::Left => "Left".into(),
            KeyCode::Right => "Right".into(),
            KeyCode::Home => "Home".into(),
            KeyCode::End => "End".into(),
            KeyCode::Char(ch) => ch.to_string(),
            other => format!("{other:?}"),
        };
        s
    }
}

#[derive(Clone, Copy, Debug)]
pub struct Binding {
    pub id: &'static str,
    pub keys: &'static [Key],
    /// Replaces the key text for gestures and key families.
    pub shown: Option<&'static str>,
    pub label: &'static str,
    /// One word for the bottom bar.
    pub hint: Option<&'static str>,
    pub scope: Scope,
    pub action: Option<Action>,
}

impl Binding {
    const fn new(
        id: &'static str,
        keys: &'static [Key],
        label: &'static str,
        scope: Scope,
        action: Option<Action>,
    ) -> Binding {
        Binding {
            id,
            keys,
            shown: None,
            label,
            hint: None,
            scope,
            action,
        }
    }

    const fn shown(mut self, text: &'static str) -> Binding {
        self.shown = Some(text);
        self
    }

    const fn hint(mut self, word: &'static str) -> Binding {
        self.hint = Some(word);
        self
    }

    /// Every key of the binding, for help and the palette.
    pub fn keys_label(&self) -> String {
        match self.shown {
            Some(s) => s.to_string(),
            None => self
                .keys
                .iter()
                .map(Key::label)
                .collect::<Vec<_>>()
                .join(", "),
        }
    }

    /// The main key, for the bottom bar and tooltips.
    pub fn first_label(&self) -> String {
        match (self.shown, self.keys.first()) {
            (Some(s), _) => s.to_string(),
            (None, Some(k)) => k.label(),
            (None, None) => String::new(),
        }
    }

    pub fn name(&self) -> &'static str {
        match self.action {
            Some(Action::Tool(t)) => t.label(),
            _ => self.label,
        }
    }
}

macro_rules! family {
    ($name:ident, $scope:expr, $variant:ident) => {
        macro_rules! $name {
            ($id:expr, $ch:expr, $label:expr, $v:expr) => {
                Binding::new($id, &[c($ch)], $label, $scope, Some(Action::$variant($v)))
            };
        }
    };
}

family!(tool, Scope::Tool, Tool);
family!(align, Scope::Selection, Align);
family!(order, Scope::Selection, Order);

const fn ex(
    id: &'static str,
    shown: &'static str,
    label: &'static str,
    action: Option<Action>,
) -> Binding {
    Binding::new(id, &[], label, Scope::Command, action).shown(shown)
}

const ARROWS: &[Key] = &[
    plain(KeyCode::Left),
    plain(KeyCode::Right),
    plain(KeyCode::Up),
    plain(KeyCode::Down),
    c('h'),
    c('j'),
    c('k'),
    c('l'),
];

const SHIFT_ARROWS: &[Key] = &[
    with(KeyCode::Left, KeyModifiers::SHIFT),
    with(KeyCode::Right, KeyModifiers::SHIFT),
    with(KeyCode::Up, KeyModifiers::SHIFT),
    with(KeyCode::Down, KeyModifiers::SHIFT),
];

const SHIFT_CARET: &[Key] = &[
    with(KeyCode::Left, KeyModifiers::SHIFT),
    with(KeyCode::Right, KeyModifiers::SHIFT),
    with(KeyCode::Home, KeyModifiers::SHIFT),
    with(KeyCode::End, KeyModifiers::SHIFT),
];

const DELETE: &[Key] = &[plain(KeyCode::Delete), plain(KeyCode::Backspace)];

/// Palette order: the palette lists the bindings with an action as they stand here.
pub const BINDINGS: &[Binding] = &[
    Binding::new(
        "undo",
        &[c('u')],
        "Undo",
        Scope::Canvas,
        Some(Action::Ex("u")),
    ),
    Binding::new(
        "redo",
        &[ctrl('r')],
        "Redo",
        Scope::Canvas,
        Some(Action::Ex("redo")),
    ),
    Binding::new(
        "copy",
        &[c('y')],
        "Copy",
        Scope::Selection,
        Some(Action::Copy),
    )
    .hint("copy"),
    Binding::new("cut", &[c('x')], "Cut", Scope::Selection, Some(Action::Cut)),
    Binding::new(
        "paste",
        &[c('p')],
        "Paste",
        Scope::Canvas,
        Some(Action::Context(ContextItem::Paste)),
    ),
    Binding::new(
        "duplicate",
        &[],
        "Duplicate",
        Scope::Palette,
        Some(Action::Context(ContextItem::Duplicate)),
    ),
    Binding::new(
        "delete",
        DELETE,
        "Delete",
        Scope::Selection,
        Some(Action::Context(ContextItem::Delete)),
    )
    .shown("Del")
    .hint("delete"),
    Binding::new(
        "edit_label",
        &[],
        "Edit label",
        Scope::Palette,
        Some(Action::Context(ContextItem::EditLabel)),
    ),
    Binding::new(
        "select_all",
        &[ctrl('a')],
        "Select all",
        Scope::Canvas,
        Some(Action::Context(ContextItem::SelectAll)),
    ),
    Binding::new(
        "group",
        &[c('g')],
        "Group",
        Scope::Selection,
        Some(Action::Group),
    )
    .hint("group"),
    Binding::new(
        "ungroup",
        &[c('G')],
        "Ungroup",
        Scope::Selection,
        Some(Action::Ungroup),
    ),
    Binding::new(
        "bold",
        &[ctrl('b')],
        "Toggle bold",
        Scope::Selection,
        Some(Action::Bold),
    ),
    Binding::new(
        "head_none",
        &[],
        "none",
        Scope::Palette,
        Some(Action::Style(Choice::Head(LineKind::Line))),
    ),
    Binding::new(
        "head_end",
        &[],
        "end",
        Scope::Palette,
        Some(Action::Style(Choice::Head(LineKind::Arrow))),
    ),
    Binding::new(
        "head_start",
        &[],
        "start",
        Scope::Palette,
        Some(Action::Style(Choice::Head(LineKind::ArrowStart))),
    ),
    Binding::new(
        "head_both",
        &[],
        "both",
        Scope::Palette,
        Some(Action::Style(Choice::Head(LineKind::ArrowBoth))),
    ),
    Binding::new(
        "stroke_solid",
        &[],
        "solid",
        Scope::Palette,
        Some(Action::Style(Choice::Stroke(Stroke::Solid))),
    ),
    Binding::new(
        "stroke_dashed",
        &[],
        "dashed",
        Scope::Palette,
        Some(Action::Style(Choice::Stroke(Stroke::Dashed))),
    ),
    Binding::new(
        "stroke_dotted",
        &[],
        "dotted",
        Scope::Palette,
        Some(Action::Style(Choice::Stroke(Stroke::Dotted))),
    ),
    Binding::new(
        "border_single",
        &[],
        "single",
        Scope::Palette,
        Some(Action::Style(Choice::Border(Border::Single))),
    ),
    Binding::new(
        "border_rounded",
        &[],
        "rounded",
        Scope::Palette,
        Some(Action::Style(Choice::Border(Border::Rounded))),
    ),
    Binding::new(
        "border_double",
        &[],
        "double",
        Scope::Palette,
        Some(Action::Style(Choice::Border(Border::Double))),
    ),
    Binding::new(
        "border_heavy",
        &[],
        "heavy",
        Scope::Palette,
        Some(Action::Style(Choice::Border(Border::Heavy))),
    ),
    Binding::new(
        "text_left",
        &[],
        "left",
        Scope::Palette,
        Some(Action::Style(Choice::Align(TextAlign::Left))),
    ),
    Binding::new(
        "text_center",
        &[],
        "center",
        Scope::Palette,
        Some(Action::Style(Choice::Align(TextAlign::Center))),
    ),
    Binding::new(
        "text_right",
        &[],
        "right",
        Scope::Palette,
        Some(Action::Style(Choice::Align(TextAlign::Right))),
    ),
    Binding::new(
        "ink",
        &[],
        "color",
        Scope::Palette,
        Some(Action::Pick(Choice::Ink)),
    ),
    Binding::new(
        "fill",
        &[],
        "color",
        Scope::Palette,
        Some(Action::Pick(Choice::Fill)),
    ),
    align!("align_left", '<', "Align left", Align::Left),
    align!("align_center", '|', "Align center", Align::Center),
    align!("align_right", '>', "Align right", Align::Right),
    align!("align_top", '^', "Align top", Align::Top),
    align!("align_middle", '-', "Align middle", Align::Middle),
    align!("align_bottom", '_', "Align bottom", Align::Bottom),
    align!(
        "distribute_h",
        '=',
        "Distribute horizontally",
        Align::DistributeH
    ),
    align!(
        "distribute_v",
        '#',
        "Distribute vertically",
        Align::DistributeV
    ),
    order!("front", '}', "Bring to front", Z::Front),
    order!("back", '{', "Send to back", Z::Back),
    order!("raise", ']', "Raise", Z::Forward),
    order!("lower", '[', "Lower", Z::Backward),
    Binding::new(
        "fit",
        &[c('z')],
        "Fit all",
        Scope::Canvas,
        Some(Action::Menu(MenuItem::FitAll)),
    )
    .hint("fit"),
    Binding::new(
        "minimap",
        &[],
        "Show minimap",
        Scope::Canvas,
        Some(Action::Map),
    )
    .shown("Ctrl-p"),
    Binding::new(
        "zoom_in",
        &[],
        "Zoom in",
        Scope::Palette,
        Some(Action::ZoomIn),
    )
    .shown(ZOOM_WHEEL),
    Binding::new(
        "zoom_out",
        &[],
        "Zoom out",
        Scope::Palette,
        Some(Action::ZoomOut),
    )
    .shown(ZOOM_WHEEL),
    Binding::new(
        "present",
        &[],
        "Present",
        Scope::Canvas,
        Some(Action::Present),
    )
    .shown("Ctrl-p"),
    Binding::new("find", &[c('/')], "Find", Scope::Canvas, Some(Action::Find)).hint("find"),
    Binding::new(
        "canvases",
        &[],
        "Switch canvas",
        Scope::Palette,
        Some(Action::Menu(MenuItem::Canvases)),
    ),
    Binding::new(
        "new_canvas",
        &[],
        "Create canvas",
        Scope::Palette,
        Some(Action::NewCanvas),
    ),
    Binding::new(
        "rename_canvas",
        &[],
        "Rename canvas",
        Scope::Palette,
        Some(Action::RenameCanvas),
    ),
    Binding::new(
        "delete_canvas",
        &[],
        "Delete canvas",
        Scope::Palette,
        Some(Action::DeleteCanvas),
    ),
    Binding::new(
        "export_svg",
        &[],
        "Export SVG",
        Scope::Palette,
        Some(Action::Menu(MenuItem::ExportSvg)),
    ),
    Binding::new(
        "export_mermaid",
        &[],
        "Export Mermaid",
        Scope::Palette,
        Some(Action::Menu(MenuItem::ExportMermaid)),
    ),
    Binding::new(
        "export_text",
        &[],
        "Export text",
        Scope::Palette,
        Some(Action::Menu(MenuItem::ExportText)),
    ),
    ex("save", ":w", "Save", Some(Action::Ex("w"))),
    ex(
        "layout_td",
        ":layout [td|lr]",
        "Layout top-down",
        Some(Action::Ex("layout")),
    ),
    Binding::new(
        "layout_lr",
        &[],
        "Layout left-right",
        Scope::Palette,
        Some(Action::Ex("layout lr")),
    )
    .shown(":layout lr"),
    ex(
        "perf",
        ":perf",
        "Toggle perf overlay",
        Some(Action::Ex("perf")),
    ),
    Binding::new(
        "help",
        &[c('?')],
        "Show shortcuts",
        Scope::Canvas,
        Some(Action::Help),
    )
    .hint("help"),
    Binding::new(
        "quit",
        &[ctrl('c')],
        "Quit",
        Scope::Canvas,
        Some(Action::Menu(MenuItem::Quit)),
    ),
    Binding::new(
        "cancel",
        &[plain(KeyCode::Esc)],
        "Cancel, deselect",
        Scope::Canvas,
        None,
    )
    .hint("cancel"),
    Binding::new("move", ARROWS, "Move or pan", Scope::Canvas, None).shown("Arrows, h j k l"),
    Binding::new(
        "connect",
        SHIFT_ARROWS,
        "Connect to the next shape",
        Scope::Canvas,
        None,
    )
    .shown("Shift-arrows")
    .hint("connect"),
    Binding::new("zoom_wheel", &[], "Zoom, 25% to 300%", Scope::Canvas, None)
        .shown(ZOOM_WHEEL)
        .hint("zoom"),
    Binding::new(
        "palette",
        &[ctrl('p')],
        "Command palette",
        Scope::Canvas,
        None,
    ),
    Binding::new("command", &[c(':')], "Command line", Scope::Canvas, None).hint("command"),
    Binding::new(
        "reset_zoom",
        &[plain(KeyCode::Esc)],
        "Back to 1x",
        Scope::ZoomedOut,
        None,
    )
    .hint("1x"),
    Binding::new("draw", &[], "Draw", Scope::Overlay, None)
        .shown("drag")
        .hint("draw"),
    Binding::new(
        "pick_area",
        &[],
        "Pick an export area",
        Scope::Overlay,
        None,
    )
    .shown("drag")
    .hint("area"),
    Binding::new("jump", &[], "Jump", Scope::Overlay, None)
        .shown("click")
        .hint("jump"),
    Binding::new(
        "confirm",
        &[plain(KeyCode::Enter)],
        "Confirm",
        Scope::Overlay,
        None,
    )
    .hint("confirm"),
    Binding::new(
        "cancel_prompt",
        &[plain(KeyCode::Esc)],
        "Cancel",
        Scope::Overlay,
        None,
    )
    .hint("cancel"),
    Binding::new(
        "apply_fix",
        &[plain(KeyCode::Tab)],
        "Apply fix",
        Scope::Overlay,
        None,
    )
    .hint("fix"),
    Binding::new("tabs", &[], "Switch tab", Scope::Overlay, None)
        .shown("‹ ›")
        .hint("switch tab"),
    Binding::new("run", &[plain(KeyCode::Enter)], "Run", Scope::Overlay, None).hint("run"),
    Binding::new(
        "select",
        &[plain(KeyCode::Up), plain(KeyCode::Down)],
        "Select",
        Scope::Overlay,
        None,
    )
    .shown("Up/Down")
    .hint("select"),
    Binding::new(
        "close",
        &[plain(KeyCode::Esc)],
        "Close",
        Scope::Overlay,
        None,
    )
    .hint("close"),
    Binding::new("type", &[], "Insert at the caret", Scope::TextEdit, None).shown("Type, paste"),
    Binding::new(
        "caret",
        &[plain(KeyCode::Left), plain(KeyCode::Right)],
        "Move the caret",
        Scope::TextEdit,
        None,
    ),
    Binding::new(
        "caret_ends",
        &[plain(KeyCode::Home), plain(KeyCode::End)],
        "Jump to the start, end",
        Scope::TextEdit,
        None,
    ),
    Binding::new(
        "word",
        &[
            with(KeyCode::Left, KeyModifiers::CONTROL),
            with(KeyCode::Right, KeyModifiers::CONTROL),
        ],
        "Jump by word",
        Scope::TextEdit,
        None,
    ),
    Binding::new(
        "select_text",
        SHIFT_CARET,
        "Select text",
        Scope::TextEdit,
        None,
    )
    .shown("Shift-arrows"),
    Binding::new(
        "delete_char",
        &[plain(KeyCode::Backspace), plain(KeyCode::Delete)],
        "Delete a character",
        Scope::TextEdit,
        None,
    ),
    Binding::new(
        "delete_word",
        &[ctrl('w')],
        "Delete word",
        Scope::TextEdit,
        None,
    ),
    Binding::new(
        "delete_to_start",
        &[ctrl('u')],
        "Delete to start",
        Scope::TextEdit,
        None,
    ),
    Binding::new(
        "newline",
        &[with(KeyCode::Enter, KeyModifiers::ALT), ctrl('j')],
        "New line in a shape label",
        Scope::TextEdit,
        None,
    )
    .hint("new line"),
    Binding::new(
        "commit_edit",
        &[plain(KeyCode::Enter)],
        "Commit the label",
        Scope::TextEdit,
        None,
    )
    .hint("commit"),
    Binding::new(
        "discard_edit",
        &[plain(KeyCode::Esc)],
        "Discard the edit",
        Scope::TextEdit,
        None,
    )
    .hint("discard"),
    tool!("tool_select", 'v', "", Tool::Select).hint("select"),
    tool!("tool_rect", 'r', "", Tool::Rect),
    tool!("tool_diamond", 'd', "", Tool::Diamond),
    tool!("tool_cylinder", 'c', "", Tool::Cylinder),
    tool!("tool_slant", 's', "", Tool::Parallelogram),
    tool!("tool_hexagon", 'e', "", Tool::Hexagon),
    tool!("tool_frame", 'f', "", Tool::Frame),
    tool!("tool_arrow", 'a', "", Tool::Arrow),
    tool!("tool_text", 't', "", Tool::Text),
    ex("quit_cmd", ":q", "Quit", None),
    ex("undo_cmd", ":u", "Undo", None),
    ex("redo_cmd", ":redo", "Redo", None),
    ex("save_quit", ":wq", "Save and quit", None),
    ex("open", ":e <name>", "Open or create a canvas", None),
    ex(
        "import",
        ":import <path>",
        "Import file",
        Some(Action::Prefill("import ")),
    ),
    ex("mermaid", ":mermaid", "Edit a Mermaid line", None),
    ex("export", ":export [svg|mmd|txt]", "Export the canvas", None),
    ex(
        "link",
        ":link <url>",
        "Link selection to URL",
        Some(Action::Prefill("link ")),
    ),
    ex(
        "lib",
        ":lib save|insert <name>",
        "Use library",
        Some(Action::Prefill("lib ")),
    ),
];

pub fn by_id(id: &str) -> &'static Binding {
    BINDINGS
        .iter()
        .find(|b| b.id == id)
        .unwrap_or_else(|| panic!("no binding {id}"))
}

pub fn in_scope(scope: Scope) -> impl Iterator<Item = &'static Binding> {
    BINDINGS.iter().filter(move |b| b.scope == scope)
}

/// The command-line verbs, in registry order.
pub fn commands() -> impl Iterator<Item = &'static Binding> {
    in_scope(Scope::Command).filter(|b| b.shown.is_some_and(|s| s.starts_with(':')))
}

/// The text a completion puts in the command line: the verb, plus a space when it takes an argument.
pub fn fill(b: &Binding) -> String {
    let shown = b.shown.unwrap_or("").trim_start_matches(':');
    let (word, rest) = shown.split_once(' ').unwrap_or((shown, ""));
    if rest.is_empty() {
        word.into()
    } else {
        format!("{word} ")
    }
}

pub fn for_action(action: Action) -> Option<&'static Binding> {
    BINDINGS.iter().find(|b| b.action == Some(action))
}

pub fn tool_key(t: Tool) -> Option<char> {
    match for_action(Action::Tool(t))?.keys.first()?.code {
        KeyCode::Char(ch) => Some(ch),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ids_are_unique() {
        for (i, b) in BINDINGS.iter().enumerate() {
            assert!(BINDINGS[..i].iter().all(|o| o.id != b.id), "{}", b.id);
        }
    }

    #[test]
    fn key_labels_use_the_dash_notation() {
        assert_eq!(ctrl('a').label(), "Ctrl-a");
        assert_eq!(with(KeyCode::Enter, KeyModifiers::ALT).label(), "Alt-Enter");
        assert_eq!(plain(KeyCode::Delete).label(), "Del");
        assert_eq!(by_id("redo").keys_label(), "Ctrl-r");
        assert_eq!(by_id("zoom_in").keys_label(), "Ctrl-wheel");
    }
}
