use super::common::*;
use clidraw::app::App;
use clidraw::model::{
    Border, Command, Element, End, Id, Line, LineKind, Paint, Shape, ShapeKind, Stroke, TextAlign,
};
use clidraw::store::Store;
use crossterm::event::KeyCode;
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use std::time::{Duration, Instant};

#[test]
fn question_mark_types_into_a_label_and_a_prompt() {
    let t = tempfile::tempdir().unwrap();
    let mut app = app_in(t.path());
    let now = Instant::now();
    click(&mut app, 30, 10, now);
    click(&mut app, 30, 10, now + Duration::from_millis(50));
    app.on_event(key(KeyCode::Char('?')), now);
    assert!(matches!(app.overlay, clidraw::ui::Overlay::None));
    assert_eq!(app.session.edit.as_ref().unwrap().buf, "?");
    app.on_event(key(KeyCode::Esc), now);
    open_canvases(&mut app, now);
    click(&mut app, 22, 7, now);
    for _ in 0..40 {
        app.on_event(key(KeyCode::Backspace), now);
    }
    app.on_event(key(KeyCode::Char('?')), now);
    let clidraw::ui::Overlay::Prompt(p) = &app.overlay else {
        panic!("expected a prompt, got another overlay")
    };
    assert_eq!(p.buf(), "?");
}

#[test]
fn new_canvas_prompt_is_prefilled_with_a_generated_name() {
    let t = tempfile::tempdir().unwrap();
    let mut app = app_in(t.path());
    let now = Instant::now();
    open_canvases(&mut app, now);
    click(&mut app, 25, 7, now);
    let clidraw::ui::Overlay::Prompt(p) = &app.overlay else {
        panic!("no prompt");
    };
    let name = p.buf().to_string();
    let (adj, noun) = name.split_once('-').expect("adjective-noun");
    assert!(!adj.is_empty() && !noun.is_empty() && name != "demo");
    app.on_event(key(KeyCode::Enter), now);
    app.on_event(key(KeyCode::Enter), now);
    assert_eq!(app.name, name);
}

#[test]
fn palette_lists_only_actions_that_can_act() {
    let t = tempfile::tempdir().unwrap();
    let mut app = app_in(t.path());
    let now = Instant::now();
    ctrl_key(&mut app, 'p', now);
    let l = palette_labels(&app);
    assert_eq!(l[0], "Edit: Undo");
    assert!(l.contains(&"Tool: Rect".to_string()));
    for gone in [
        "Edit: Duplicate",
        "Arrange: Align left",
        "Edit: Paste",
        "Edit: Edit label",
    ] {
        assert!(!l.contains(&gone.to_string()), "{gone}");
    }
    app.on_event(key(KeyCode::Esc), now);
    assert!(!palette_open(&app));
    app.session.exec(shape_at("aaaaaaaa", 20, 5));
    app.session.selection = vec![Id::from("aaaaaaaa")];
    ctrl_key(&mut app, 'p', now);
    let l = palette_labels(&app);
    for here in ["Edit: Duplicate", "Arrange: Align left", "Edit: Edit label"] {
        assert!(l.contains(&here.to_string()), "{here}");
    }
    assert!(!l.contains(&"Edit: Paste".to_string()));
    app.on_event(key(KeyCode::Esc), now);
    app.session.copy();
    ctrl_key(&mut app, 'p', now);
    assert!(palette_labels(&app).contains(&"Edit: Paste".to_string()));
}

#[test]
fn palette_zoom_actions_step_the_zoom_around_the_screen_center() {
    let t = tempfile::tempdir().unwrap();
    let mut app = app_in(t.path());
    let now = Instant::now();
    ctrl_key(&mut app, 'p', now);
    type_str(&mut app, "zoom in", now);
    app.on_event(key(KeyCode::Enter), now);
    assert_eq!(app.session.view.zoom, clidraw::view::Zoom::Two);
    ctrl_key(&mut app, 'p', now);
    type_str(&mut app, "zoom out", now);
    app.on_event(key(KeyCode::Enter), now);
    assert_eq!(app.session.view.zoom, clidraw::view::Zoom::One);
}

#[test]
fn palette_lists_save_quit_and_mermaid_editor() {
    let t = tempfile::tempdir().unwrap();
    let mut app = app_in(t.path());
    let now = Instant::now();
    ctrl_key(&mut app, 'p', now);
    let l = palette_labels(&app);
    assert!(l.contains(&"File: Save and quit".to_string()), "{l:?}");
    assert!(
        l.contains(&"Edit: Edit a Mermaid line".to_string()),
        "{l:?}"
    );
}

#[test]
fn palette_mermaid_opens_editor_and_save_quit_quits() {
    let t = tempfile::tempdir().unwrap();
    let mut app = app_in(t.path());
    let now = Instant::now();
    ctrl_key(&mut app, 'p', now);
    type_str(&mut app, "mermaid line", now);
    app.on_event(key(KeyCode::Enter), now);
    assert!(matches!(app.overlay, clidraw::ui::Overlay::Prompt(_)));
    app.on_event(key(KeyCode::Esc), now);
    ctrl_key(&mut app, 'p', now);
    type_str(&mut app, "save and quit", now);
    app.on_event(key(KeyCode::Enter), now);
    assert!(app.quit);
}

#[test]
fn palette_filters_fuzzily_and_backspace_widens() {
    let t = tempfile::tempdir().unwrap();
    let mut app = app_in(t.path());
    let now = Instant::now();
    app.session.exec(shape_at("aaaaaaaa", 20, 5));
    app.session.selection = vec![Id::from("aaaaaaaa")];
    ctrl_key(&mut app, 'p', now);
    type_str(&mut app, "duplicate", now);
    assert_eq!(palette_labels(&app)[0], "Edit: Duplicate");
    app.on_event(key(KeyCode::Esc), now);
    ctrl_key(&mut app, 'p', now);
    type_str(&mut app, "exsvg", now);
    assert_eq!(palette_labels(&app)[0], "File: Export SVG");
    type_str(&mut app, "zzz", now);
    assert!(palette_labels(&app).is_empty());
    app.on_event(key(KeyCode::Enter), now);
    assert!(palette_open(&app));
    for _ in 0..3 {
        app.on_event(key(KeyCode::Backspace), now);
    }
    assert_eq!(palette_labels(&app)[0], "File: Export SVG");
}

#[test]
fn palette_finds_synonyms_below_direct_hits() {
    let t = tempfile::tempdir().unwrap();
    let mut app = app_in(t.path());
    let now = Instant::now();
    ctrl_key(&mut app, 'p', now);
    type_str(&mut app, "new", now);
    assert!(palette_labels(&app).contains(&"Canvas: Create canvas".to_string()));
    app.on_event(key(KeyCode::Esc), now);
    ctrl_key(&mut app, 'p', now);
    type_str(&mut app, "list", now);
    assert!(palette_labels(&app).contains(&"Canvas: Switch canvas".to_string()));
    app.on_event(key(KeyCode::Esc), now);
    ctrl_key(&mut app, 'p', now);
    type_str(&mut app, "switch canvas", now);
    assert_eq!(palette_labels(&app)[0], "Canvas: Switch canvas");
}

#[test]
fn palette_enter_runs_the_entry() {
    let t = tempfile::tempdir().unwrap();
    let mut app = app_in(t.path());
    let now = Instant::now();
    ctrl_key(&mut app, 'p', now);
    type_str(&mut app, "rect", now);
    app.on_event(key(KeyCode::Enter), now);
    assert!(!palette_open(&app));
    assert_eq!(app.session.tool, clidraw::input::Tool::Rect);

    app.session.exec(shape_at("aaaaaaaa", 20, 5));
    app.session.selection = vec![Id::from("aaaaaaaa")];
    ctrl_key(&mut app, 'p', now);
    type_str(&mut app, "edit: delete", now);
    app.on_event(key(KeyCode::Enter), now);
    assert!(app.session.canvas.elements.is_empty());
    app.on_event(key(KeyCode::Char('u')), now);
    assert_eq!(app.session.canvas.elements.len(), 1);

    ctrl_key(&mut app, 'p', now);
    type_str(&mut app, "export svg", now);
    app.on_event(key(KeyCode::Enter), now);
    assert!(matches!(app.overlay, clidraw::ui::Overlay::Scope(..)));
    app.on_event(key(KeyCode::Esc), now);

    ctrl_key(&mut app, 'p', now);
    type_str(&mut app, "perf", now);
    app.on_event(key(KeyCode::Enter), now);
    assert!(app.perf.is_some());

    ctrl_key(&mut app, 'p', now);
    app.on_event(key(KeyCode::Down), now);
    app.on_event(key(KeyCode::Enter), now);
    assert!(!palette_open(&app));
    assert!(app.session.canvas.elements.is_empty());
}

#[test]
fn palette_clicks_run_rows_and_outside_closes() {
    let t = tempfile::tempdir().unwrap();
    let mut app = app_in(t.path());
    let now = Instant::now();
    ctrl_key(&mut app, 'p', now);
    let third = palette_labels(&app)[2].clone();
    assert_eq!(third, "Edit: Select all");
    // 80x24 box is 13 rows high at y=5; rows start at y+2.
    click(&mut app, 40, 5 + 2 + 2, now);
    assert!(!palette_open(&app));
    ctrl_key(&mut app, 'p', now);
    click(&mut app, 1, 1, now);
    assert!(!palette_open(&app));
}

#[test]
fn palette_ex_line_hides_rows_and_esc_restores_them() {
    let t = tempfile::tempdir().unwrap();
    let mut app = app_in(t.path());
    let now = Instant::now();
    ctrl_key(&mut app, 'p', now);
    type_str(&mut app, ":w", now);
    assert!(palette_labels(&app).is_empty());
    assert!(palette_open(&app));
    app.on_event(key(KeyCode::Esc), now);
    assert!(!palette_labels(&app).is_empty());
    assert!(palette_open(&app));
}

#[test]
fn palette_ex_line_shows_the_run_hint() {
    let t = tempfile::tempdir().unwrap();
    let mut app = app_in(t.path());
    let now = Instant::now();
    ctrl_key(&mut app, 'p', now);
    type_str(&mut app, ":w", now);
    let area = Rect::new(0, 0, 80, 24);
    let mut buf = Buffer::empty(area);
    app.render(&mut buf, area);
    let screen: String = (0..24)
        .flat_map(|y| (0..80).map(move |x| (x, y)))
        .map(|(x, y)| buf[(x, y)].symbol().to_string())
        .collect();
    assert!(screen.contains("Enter: run"));
}

#[test]
fn palette_ex_line_wq_saves_and_quits() {
    let t = tempfile::tempdir().unwrap();
    let mut app = app_in(t.path());
    let file = t.path().join("c/demo.clidraw.json");
    let empty = std::fs::read(&file).unwrap();
    insert_shape(&mut app);
    let now = Instant::now();
    ctrl_key(&mut app, 'p', now);
    type_str(&mut app, ":wq", now);
    app.on_event(key(KeyCode::Enter), now);
    assert_ne!(std::fs::read(&file).unwrap(), empty);
    assert!(app.quit);
    assert!(!palette_open(&app));
}

#[test]
fn palette_ex_line_e_switches_canvas() {
    let t = tempfile::tempdir().unwrap();
    let mut app = app_in(t.path());
    let now = Instant::now();
    run_palette(&mut app, ":e other", now);
    assert_eq!(app.name, "other");
    assert!(!palette_open(&app));
}

#[test]
fn palette_ex_line_slash_selects_the_match() {
    let t = tempfile::tempdir().unwrap();
    let mut app = app_in(t.path());
    app.session.exec(labeled("aaaaaaaa", 500, "Auth service"));
    app.session.exec(labeled("bbbbbbbb", 0, "db"));
    let now = Instant::now();
    run_palette(&mut app, "/auth", now);
    assert_eq!(app.session.selection, vec![Id::from("aaaaaaaa")]);
    assert!(!palette_open(&app));
}

#[test]
fn palette_bare_colon_closes_without_a_status_error() {
    let t = tempfile::tempdir().unwrap();
    let mut app = app_in(t.path());
    insert_shape(&mut app);
    let now = Instant::now();
    run_palette(&mut app, ":", now);
    assert!(!palette_open(&app));
    assert_eq!(app.session.canvas.elements.len(), 1);
}

#[test]
fn palette_template_list_keeps_colon_as_fuzzy_text() {
    let t = tempfile::tempdir().unwrap();
    let mut app = app_in(t.path());
    let now = Instant::now();
    run_palette(&mut app, "insert template", now);
    type_str(&mut app, ":", now);
    assert!(palette_labels(&app).is_empty());
    app.on_event(key(KeyCode::Enter), now);
    assert!(palette_open(&app));
    assert!(app.session.canvas.elements.is_empty());
}

#[test]
fn palette_does_not_open_during_an_edit_and_ctrl_c_quits() {
    let t = tempfile::tempdir().unwrap();
    let mut app = app_in(t.path());
    let now = Instant::now();
    app.session.exec(shape_at("aaaaaaaa", 20, 5));
    app.session.start_edit_existing(Id::from("aaaaaaaa"));
    ctrl_key(&mut app, 'p', now);
    assert!(!palette_open(&app));
    assert!(app.session.edit.is_some());
    app.on_event(key(KeyCode::Esc), now);
    ctrl_key(&mut app, 'p', now);
    ctrl_key(&mut app, 'c', now);
    assert!(app.quit);
}

#[test]
fn palette_ctrl_n_and_ctrl_p_move_the_selection() {
    let t = tempfile::tempdir().unwrap();
    let mut app = app_in(t.path());
    let now = Instant::now();
    let selected = |app: &App| match &app.overlay {
        clidraw::ui::Overlay::Actions(a) => a.selected(),
        _ => panic!("palette is not open"),
    };
    ctrl_key(&mut app, 'p', now);
    let first = selected(&app);
    ctrl_key(&mut app, 'n', now);
    let second = selected(&app);
    assert_ne!(first, second);
    ctrl_key(&mut app, 'p', now);
    assert_eq!(selected(&app), first);
    assert!(palette_open(&app));
    app.on_event(key(KeyCode::Down), now);
    assert_eq!(selected(&app), second);
}

#[test]
fn switcher_renames_and_deletes_the_highlighted_canvas_from_the_keyboard() {
    use clidraw::ui::prompt::PromptKind;
    let t = tempfile::tempdir().unwrap();
    let mut app = app_in(t.path());
    let now = Instant::now();
    open_canvases(&mut app, now);
    ctrl_key(&mut app, 'x', now);
    assert!(matches!(prompt_kind(&app), Some(PromptKind::ConfirmDelete(n)) if n == app.name));
    app.on_event(key(KeyCode::Esc), now);
    open_canvases(&mut app, now);
    ctrl_key(&mut app, 'r', now);
    assert!(matches!(prompt_kind(&app), Some(PromptKind::Rename(n)) if n == app.name));
}

#[test]
fn switcher_filters_by_typed_text_and_plain_r_x_do_not_rename_or_delete() {
    let t = tempfile::tempdir().unwrap();
    let mut app = app_in(t.path());
    let now = Instant::now();
    Store::at(t.path().join("c"), t.path().join("state"))
        .create("other")
        .unwrap();
    open_canvases(&mut app, now);
    type_str(&mut app, "rx", now);
    assert!(prompt_kind(&app).is_none());
    let clidraw::ui::Overlay::Switcher(sw) = &app.overlay else {
        panic!("expected the switcher")
    };
    assert!(sw.list.labels().is_empty());
    for _ in 0..2 {
        app.on_event(key(KeyCode::Backspace), now);
    }
    type_str(&mut app, "oth", now);
    let clidraw::ui::Overlay::Switcher(sw) = &app.overlay else {
        panic!("expected the switcher")
    };
    assert_eq!(sw.list.labels(), ["other"]);
    app.on_event(key(KeyCode::Enter), now);
    assert_eq!(app.name, "other");
}

#[test]
fn rename_and_delete_move_and_drop_the_saved_viewport() {
    let t = tempfile::tempdir().unwrap();
    let store = Store::at(t.path().join("c"), t.path().join("state"));
    let mut seed = clidraw::store::State::default();
    seed.offsets.insert("demo".into(), (7, 9));
    store.save_state(&seed).unwrap();
    let mut app = app_in(t.path());
    let now = Instant::now();
    open_canvases(&mut app, now);
    ctrl_key(&mut app, 'r', now);
    type_str(&mut app, "2", now);
    app.on_event(key(KeyCode::Enter), now);
    let state = store.load_state();
    assert_eq!(state.offsets.get("demo2"), Some(&(7, 9)));
    assert!(!state.offsets.contains_key("demo"));
    ctrl_key(&mut app, 'x', now);
    app.on_event(key(KeyCode::Enter), now);
    assert!(!store.load_state().offsets.contains_key("demo2"));
}

#[test]
fn switcher_moves_with_arrows_and_enter_on_the_current_canvas_closes() {
    use clidraw::ui::prompt::PromptKind;
    let t = tempfile::tempdir().unwrap();
    let mut app = app_in(t.path());
    let now = Instant::now();
    open_canvases(&mut app, now);
    app.on_event(key(KeyCode::Up), now);
    ctrl_key(&mut app, 'x', now);
    assert!(prompt_kind(&app).is_none(), "x on the New row does nothing");
    app.on_event(key(KeyCode::Down), now);
    ctrl_key(&mut app, 'x', now);
    assert!(matches!(
        prompt_kind(&app),
        Some(PromptKind::ConfirmDelete(_))
    ));
    app.on_event(key(KeyCode::Esc), now);
    open_canvases(&mut app, now);
    app.on_event(key(KeyCode::Enter), now);
    assert!(matches!(app.overlay, clidraw::ui::Overlay::None));
}

#[test]
fn confirm_delete_moves_focus_with_arrows_and_hl_and_enter_acts_on_it() {
    let t = tempfile::tempdir().unwrap();
    let store = Store::at(t.path().join("c"), t.path().join("state"));
    let mut app = app_in(t.path());
    let now = Instant::now();
    let name = app.name.clone();
    assert!(store.list().unwrap().contains(&name));
    open_canvases(&mut app, now);
    app.on_event(key(KeyCode::Char('x')), now);
    app.on_event(key(KeyCode::Right), now);
    app.on_event(key(KeyCode::Left), now);
    app.on_event(key(KeyCode::Char('l')), now);
    app.on_event(key(KeyCode::Enter), now);
    assert!(
        store.list().unwrap().contains(&name),
        "Enter on Cancel keeps the canvas"
    );
    open_canvases(&mut app, now);
    app.on_event(key(KeyCode::Char('x')), now);
    app.on_event(key(KeyCode::Char('l')), now);
    app.on_event(key(KeyCode::Char('h')), now);
    app.on_event(key(KeyCode::Enter), now);
    assert!(
        !store.list().unwrap().contains(&name),
        "Enter on OK deletes the canvas"
    );
}

#[test]
fn palette_types_j_k_into_the_query_and_esc_clears_it_before_closing() {
    let t = tempfile::tempdir().unwrap();
    let mut app = app_in(t.path());
    let now = Instant::now();
    ctrl_key(&mut app, 'p', now);
    let all = palette_labels(&app).len();
    type_str(&mut app, "jk", now);
    assert!(palette_labels(&app).len() < all);
    app.on_event(key(KeyCode::Esc), now);
    assert!(palette_open(&app));
    assert_eq!(palette_labels(&app).len(), all);
    app.on_event(key(KeyCode::Esc), now);
    assert!(!palette_open(&app));
}

#[test]
fn palette_reaches_canvas_and_file_commands() {
    use clidraw::ui::prompt::PromptKind;
    let t = tempfile::tempdir().unwrap();
    let mut app = app_in(t.path());
    let now = Instant::now();
    for (query, kind) in [
        ("create canvas", "new"),
        ("rename canvas", "rename"),
        ("delete canvas", "delete"),
        ("import file", "import"),
        ("use library", "lib"),
    ] {
        ctrl_key(&mut app, 'p', now);
        type_str(&mut app, query, now);
        app.on_event(key(KeyCode::Enter), now);
        let got = match prompt_kind(&app) {
            Some(PromptKind::NewCanvas) => "new",
            Some(PromptKind::Rename(n)) if n == app.name => "rename",
            Some(PromptKind::ConfirmDelete(n)) if n == app.name => "delete",
            Some(PromptKind::Command) => kind,
            _ => "none",
        };
        assert_eq!(got, kind, "{query}");
        app.on_event(key(KeyCode::Esc), now);
    }
    ctrl_key(&mut app, 'p', now);
    assert!(!palette_labels(&app).contains(&"Edit: Link selection to URL".to_string()));
}

fn rect(app: &App) -> Shape {
    match app.session.canvas.get(&Id::from("aaaaaaaa")) {
        Some(Element::Shape(s)) => s.clone(),
        _ => panic!("no shape"),
    }
}

#[test]
fn palette_applies_the_selection_style_options_and_lists_them_only_for_a_selection() {
    let t = tempfile::tempdir().unwrap();
    let mut app = app_in(t.path());
    let now = Instant::now();
    app.session.exec(shape_at("aaaaaaaa", 20, 5));
    ctrl_key(&mut app, 'p', now);
    assert!(!palette_labels(&app).contains(&"Stroke: dashed".to_string()));
    app.on_event(key(KeyCode::Esc), now);
    app.session.selection = vec![Id::from("aaaaaaaa")];
    ctrl_key(&mut app, 'p', now);
    let l = palette_labels(&app);
    for here in [
        "Stroke: dashed",
        "Border: rounded",
        "Text: right",
        "Ink: color",
        "Fill: color",
    ] {
        assert!(l.contains(&here.to_string()), "{here}");
    }
    assert!(!l.contains(&"Head: both".to_string()));
    app.on_event(key(KeyCode::Esc), now);
    run_palette(&mut app, "stroke dashed", now);
    assert_eq!(rect(&app).stroke, Stroke::Dashed);
    run_palette(&mut app, "border rounded", now);
    assert_eq!(rect(&app).border, Border::Rounded);
    run_palette(&mut app, "text right", now);
    assert_eq!(rect(&app).align, TextAlign::Right);
    app.session.undo();
    assert_eq!(rect(&app).align, TextAlign::Center);
}

#[test]
fn palette_head_both_sets_the_selected_line() {
    let t = tempfile::tempdir().unwrap();
    let mut app = app_in(t.path());
    let now = Instant::now();
    app.session.exec(Command::Insert(
        0,
        Element::Line(Line {
            id: Id::from("llllllll"),
            kind: LineKind::Line,
            from: End::Free(5, 5),
            to: End::Free(25, 5),
            label: String::new(),
            label_at: None,
            stroke: Default::default(),
            bends: vec![],
            color: None,
            meta: Default::default(),
        }),
    ));
    app.session.selection = vec![Id::from("llllllll")];
    run_palette(&mut app, "head both", now);
    match app.session.canvas.get(&Id::from("llllllll")) {
        Some(Element::Line(l)) => assert_eq!(l.kind, LineKind::ArrowBoth),
        _ => panic!("no line"),
    }
}

#[test]
fn palette_ink_opens_a_swatch_grid_that_enter_applies_and_esc_leaves() {
    let t = tempfile::tempdir().unwrap();
    let mut app = app_in(t.path());
    let now = Instant::now();
    app.session.exec(shape_at("aaaaaaaa", 20, 5));
    app.session.selection = vec![Id::from("aaaaaaaa")];
    run_palette(&mut app, "ink color", now);
    app.on_event(key(KeyCode::Esc), now);
    assert_eq!(rect(&app).color, None);
    run_palette(&mut app, "ink color", now);
    app.on_event(key(KeyCode::Right), now);
    app.on_event(key(KeyCode::Right), now);
    app.on_event(key(KeyCode::Enter), now);
    assert_eq!(rect(&app).color, Some(Paint::Orange));
    run_palette(&mut app, "fill color", now);
    app.on_event(key(KeyCode::Right), now);
    app.on_event(key(KeyCode::Enter), now);
    assert_eq!(rect(&app).fill, Some(Paint::Red));
}

#[test]
fn palette_ink_takes_a_typed_hex_on_enter() {
    let t = tempfile::tempdir().unwrap();
    let mut app = app_in(t.path());
    let now = Instant::now();
    app.session.exec(shape_at("aaaaaaaa", 20, 5));
    app.session.selection = vec![Id::from("aaaaaaaa")];
    run_palette(&mut app, "ink color", now);
    type_str(&mut app, "#00ff88", now);
    app.on_event(key(KeyCode::Enter), now);
    assert_eq!(rect(&app).color, Some(Paint::Rgb(0, 255, 136)));
    assert!(!palette_open(&app));
}

/// Finds `text` on the rendered 80x24 screen, from the bottom row up, as (column, row).
fn text_at(app: &App, text: &str) -> (u16, u16) {
    let area = Rect::new(0, 0, 80, 24);
    let mut buf = Buffer::empty(area);
    app.render(&mut buf, area);
    (0..24)
        .rev()
        .find_map(|y| {
            let row: String = (0..80).map(|x| buf[(x, y)].symbol().to_string()).collect();
            row.find(text)
                .map(|at| (row[..at].chars().count() as u16, y))
        })
        .unwrap_or_else(|| panic!("{text} is not on screen"))
}

fn select_one_shape(app: &mut App) {
    app.session.exec(shape_at("aaaaaaaa", 20, 5));
    app.session.selection = vec![Id::from("aaaaaaaa")];
}

fn shape_kind(app: &App) -> ShapeKind {
    match app.session.canvas.get(&Id::from("aaaaaaaa")) {
        Some(Element::Shape(s)) => s.kind,
        _ => panic!("no shape"),
    }
}

#[test]
fn clicking_an_ink_swatch_paints_the_selection_and_closes_the_list() {
    let t = tempfile::tempdir().unwrap();
    let mut app = app_in(t.path());
    let now = Instant::now();
    select_one_shape(&mut app);
    ctrl_key(&mut app, 'p', now);
    type_str(&mut app, "ink", now);
    app.on_event(key(KeyCode::Enter), now);
    let (x, y) = text_at(&app, "Ink");
    click(&mut app, x + 10, y + 1, now);
    assert!(matches!(
        app.session.canvas.get(&Id::from("aaaaaaaa")),
        Some(Element::Shape(s)) if s.color == Some(Paint::Red)
    ));
    assert!(matches!(app.overlay, clidraw::ui::Overlay::None));
}

#[test]
fn clicking_ink_in_the_option_row_opens_the_swatch_window() {
    let t = tempfile::tempdir().unwrap();
    let mut app = app_in(t.path());
    let now = Instant::now();
    select_one_shape(&mut app);
    let ink = clidraw::ui::palette::PaletteHit::Option(clidraw::ui::palette::Choice::Ink);
    let cell = (0..200)
        .find_map(|scroll| {
            app.session.option_scroll = scroll;
            let opts = clidraw::ui::palette::choices(
                &clidraw::ui::palette::selected(&app.session),
                &app.session.canvas.groups,
            );
            clidraw::ui::palette::layout(&opts, 80, (0, scroll))
                .cells
                .into_iter()
                .find(|c| c.hit == ink)
        })
        .unwrap();
    click(&mut app, cell.x as u16, cell.y as u16, now);
    assert!(matches!(app.overlay, clidraw::ui::Overlay::Colors(..)));
    assert_eq!(rect(&app).color, None);
}

#[test]
fn the_swatch_grid_starts_on_the_current_color() {
    let t = tempfile::tempdir().unwrap();
    let mut app = app_in(t.path());
    let now = Instant::now();
    app.session.exec(shape_at("aaaaaaaa", 20, 5));
    app.session.selection = vec![Id::from("aaaaaaaa")];
    app.session
        .set_paint(clidraw::ui::palette::Choice::Ink, Some(Paint::Blue));
    run_palette(&mut app, "ink color", now);
    app.on_event(key(KeyCode::Enter), now);
    assert_eq!(rect(&app).color, Some(Paint::Blue));
}

#[test]
fn clicking_a_shape_kind_applies_it_and_clicking_off_the_list_restores_the_kind() {
    let t = tempfile::tempdir().unwrap();
    let mut app = app_in(t.path());
    let now = Instant::now();
    select_one_shape(&mut app);
    ctrl_key(&mut app, 'p', now);
    type_str(&mut app, "change shape", now);
    app.on_event(key(KeyCode::Enter), now);
    app.on_event(key(KeyCode::Down), now);
    assert_eq!(
        shape_kind(&app),
        ShapeKind::Diamond,
        "arrow previews the kind"
    );
    click(&mut app, 0, 0, now);
    assert_eq!(shape_kind(&app), ShapeKind::Rect);
    assert!(matches!(app.overlay, clidraw::ui::Overlay::None));

    ctrl_key(&mut app, 'p', now);
    type_str(&mut app, "change shape", now);
    app.on_event(key(KeyCode::Enter), now);
    let (x, y) = text_at(&app, "Hexagon");
    click(&mut app, x + 1, y, now);
    assert_eq!(shape_kind(&app), ShapeKind::Hexagon);
}

#[test]
fn clicking_the_rename_or_delete_text_of_a_canvas_row_starts_that_prompt() {
    use clidraw::ui::prompt::PromptKind;
    let t = tempfile::tempdir().unwrap();
    let mut app = app_in(t.path());
    let now = Instant::now();
    open_canvases(&mut app, now);
    let (x, y) = text_at(&app, "r rename");
    click(&mut app, x, y, now);
    assert!(matches!(prompt_kind(&app), Some(PromptKind::Rename(_))));
    app.on_event(key(KeyCode::Esc), now);

    open_canvases(&mut app, now);
    let (x, y) = text_at(&app, "x delete");
    click(&mut app, x, y, now);
    assert!(matches!(
        prompt_kind(&app),
        Some(PromptKind::ConfirmDelete(_))
    ));
    app.on_event(key(KeyCode::Esc), now);

    open_canvases(&mut app, now);
    app.on_event(key(KeyCode::Esc), now);
    assert!(matches!(app.overlay, clidraw::ui::Overlay::None));
}

#[test]
fn clicking_off_the_colors_scope_or_canvas_list_closes_it() {
    let t = tempfile::tempdir().unwrap();
    let mut app = app_in(t.path());
    let now = Instant::now();
    select_one_shape(&mut app);
    ctrl_key(&mut app, 'p', now);
    type_str(&mut app, "ink", now);
    app.on_event(key(KeyCode::Enter), now);
    assert!(matches!(app.overlay, clidraw::ui::Overlay::Colors(..)));
    click(&mut app, 0, 0, now);
    assert!(matches!(app.overlay, clidraw::ui::Overlay::None));

    open_export(&mut app, now);
    assert!(matches!(app.overlay, clidraw::ui::Overlay::Scope(..)));
    click(&mut app, 0, 0, now);
    assert!(matches!(app.overlay, clidraw::ui::Overlay::None));

    open_canvases(&mut app, now);
    click(&mut app, 0, 0, now);
    assert!(matches!(app.overlay, clidraw::ui::Overlay::None));
}
