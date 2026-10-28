use super::common::*;
use clidraw::app::App;
use clidraw::config::CommandUi;
use clidraw::model::Id;
use crossterm::event::{Event, KeyCode, KeyEvent};
use std::time::Instant;

fn registry_app(dir: &std::path::Path) -> App {
    let mut app = app_in(dir);
    for (id, x, y) in [
        ("aaaaaaaa", 2, 5),
        ("dddddddd", 70, 2),
        ("bbbbbbbb", 20, 8),
        ("cccccccc", 50, 20),
        ("eeeeeeee", 30, 18),
    ] {
        app.session.exec(shape_at(id, x, y));
    }
    app.session.undo();
    app.session.selection = ["aaaaaaaa", "bbbbbbbb", "cccccccc"].map(Id::from).to_vec();
    app.session.copy();
    app
}

/// Canvas, selection and mode as text, with generated ids masked.
fn world(app: &App) -> String {
    let mut out = format!(
        "{}|{:?}|{:?}|{}|{}|{:?}",
        serde_json::to_string(&app.session.canvas).unwrap(),
        app.session.selection,
        app.session.tool,
        app.quit,
        app.session.present,
        std::mem::discriminant(&app.overlay),
    );
    let fixed = ["aaaaaaaa", "bbbbbbbb", "cccccccc", "dddddddd"];
    for e in &app.session.canvas.elements {
        if !fixed.contains(&e.id().0.as_str()) {
            out = out.replace(&e.id().0, "new");
        }
    }
    out
}

fn press(app: &mut App, k: &clidraw::ui::registry::Key) {
    app.on_event(Event::Key(KeyEvent::new(k.code, k.mods)), Instant::now());
}

#[test]
fn registry_runnable_entries_dispatch_their_action() {
    use clidraw::ui::registry::BINDINGS;
    let t = tempfile::tempdir().unwrap();
    let now = Instant::now();
    let runnable = BINDINGS
        .iter()
        .filter(|b| b.action.is_some() && !b.keys.is_empty());
    for (i, b) in runnable.enumerate() {
        for (n, k) in b.keys.iter().enumerate() {
            let mut by_key = registry_app(&t.path().join(format!("k{i}-{n}")));
            let mut by_palette = registry_app(&t.path().join(format!("p{i}-{n}")));
            let start = world(&by_key);
            press(&mut by_key, k);
            ctrl_key(&mut by_palette, 'p', now);
            let label = format!("{}: {}", b.action.unwrap().category(), b.name());
            type_str(&mut by_palette, &label, now);
            by_palette.on_event(key(KeyCode::Enter), now);
            assert_eq!(world(&by_key), world(&by_palette), "{} {}", b.id, k.label());
            let inert = ["copy", "ungroup", "fit", "tool_select"];
            assert!(
                world(&by_key) != start || inert.contains(&b.id),
                "{} did nothing",
                b.id
            );
        }
    }
}

type Case = (&'static str, fn(&mut App), fn(&App) -> bool);

fn find_two(app: &mut App) {
    app.session.exec(labeled("ffffffff", 0, "find one"));
    app.session.exec(labeled("gggggggg", 0, "find two"));
    run(app, "/find");
}

fn editing(app: &mut App) {
    app.session.start_edit_existing(Id::from("aaaaaaaa"));
    let e = app.session.edit.as_mut().unwrap();
    e.buf = "hello world".into();
    e.caret = e.buf.len();
}

fn label_is(app: &App, want: &str) -> bool {
    app.session.edit.as_ref().is_some_and(|e| e.buf == want)
}

fn palette_open_at_second(app: &mut App) {
    palette_typed(app, "");
    app.on_event(key(KeyCode::Down), Instant::now());
}

fn zoomed(app: &mut App) {
    app.session.zoom_to(clidraw::view::Zoom::Half, (40, 12));
}

#[test]
fn registry_display_only_entries_are_covered() {
    use clidraw::ui::actions::Action;
    use clidraw::ui::{registry::BINDINGS, Overlay};
    let cases: Vec<Case> = vec![
        ("cancel", |_| {}, |a| a.session.selection.is_empty()),
        ("move", |_| {}, |a| world(a).contains("\"x\":1")),
        (
            "connect",
            |a| a.session.selection = vec![Id::from("bbbbbbbb")],
            |a| a.session.canvas.elements.len() == 6,
        ),
        (
            "palette",
            |_| {},
            |a| matches!(a.overlay, Overlay::Actions(_)),
        ),
        (
            "command",
            |_| {},
            |a| matches!(a.overlay, Overlay::Prompt(_)),
        ),
        ("reset_zoom", zoomed, |a| !a.session.zoomed_out()),
        ("confirm", |a| type_str(a, ":q", Instant::now()), |a| a.quit),
        ("find_next", find_two, |a| {
            a.session.selection == vec![Id::from("ffffffff")]
        }),
        (
            "find_prev",
            |a| {
                find_two(a);
                a.on_event(key(KeyCode::Char('n')), Instant::now());
            },
            |a| a.session.selection == vec![Id::from("gggggggg")],
        ),
        (
            "cancel_prompt",
            |a| type_str(a, ":", Instant::now()),
            |a| matches!(a.overlay, Overlay::None),
        ),
        (
            "apply_fix",
            |a| type_mermaid(a, "A -> B", KeyCode::Tab),
            |a| buffer(a) == "A --> B",
        ),
        (
            "overview_target",
            |a| {
                palette_typed(a, "mini");
                a.on_event(key(KeyCode::Enter), Instant::now());
            },
            |a| {
                matches!(
                    a.overlay,
                    Overlay::Map(clidraw::ui::minimap::Target::Selection)
                )
            },
        ),
        (
            "run",
            |a| palette_typed(a, "present"),
            |a| a.session.present,
        ),
        (
            "select",
            palette_open_at_second,
            |a| matches!(&a.overlay, Overlay::Actions(x) if x.selected() == Some(Action::Ex("u"))),
        ),
        (
            "close",
            |a| palette_typed(a, ""),
            |a| matches!(a.overlay, Overlay::None),
        ),
        ("caret", editing, |a| {
            a.session.edit.as_ref().is_some_and(|e| e.caret == 10)
        }),
        ("caret_ends", editing, |a| {
            a.session.edit.as_ref().is_some_and(|e| e.caret == 0)
        }),
        ("word", editing, |a| {
            a.session.edit.as_ref().is_some_and(|e| e.caret == 6)
        }),
        ("select_text", editing, |a| {
            a.session.edit.as_ref().is_some_and(|e| e.anchor.is_some())
        }),
        ("delete_char", editing, |a| label_is(a, "hello worl")),
        ("delete_word", editing, |a| label_is(a, "hello ")),
        ("delete_to_start", editing, |a| label_is(a, "")),
        ("newline", editing, |a| {
            a.session
                .edit
                .as_ref()
                .is_some_and(|e| e.buf.ends_with('\n'))
        }),
        ("commit_edit", editing, |a| {
            a.session.edit.is_none() && world(a).contains("hello world")
        }),
        ("discard_edit", editing, |a| {
            a.session.edit.is_none() && !world(a).contains("hello")
        }),
    ];
    let t = tempfile::tempdir().unwrap();
    for b in BINDINGS
        .iter()
        .filter(|b| b.action.is_none() && !b.keys.is_empty())
    {
        let Some((i, (_, setup, check))) = cases.iter().enumerate().find(|(_, c)| c.0 == b.id)
        else {
            panic!("no case for {}", b.id);
        };
        let mut app = registry_app(&t.path().join(format!("d{i}")));
        if matches!(b.id, "command" | "confirm") {
            app.session.command_ui = CommandUi::Vim;
        }
        setup(&mut app);
        press(&mut app, &b.keys[0]);
        assert!(check(&app), "{} {}", b.id, b.keys[0].label());
    }
}
