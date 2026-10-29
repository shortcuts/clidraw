use super::common::*;
use clidraw::app::App;
use clidraw::config::CommandUi;
use crossterm::event::{Event, KeyCode, KeyEvent, KeyEventKind, KeyModifiers};
use std::time::Instant;

#[test]
fn colon_q_quits_and_w_saves_without_waiting() {
    let t = tempfile::tempdir().unwrap();
    let mut app = app_in(t.path());
    let file = t.path().join("c/demo.clidraw.json");
    let empty = std::fs::read(&file).unwrap();
    insert_shape(&mut app);
    run(&mut app, "w");
    assert_ne!(std::fs::read(&file).unwrap(), empty);
    assert!(!app.quit);
    run(&mut app, "q");
    assert!(app.quit);
}

#[test]
fn quitting_during_a_text_edit_saves_the_typed_text() {
    let t = tempfile::tempdir().unwrap();
    let mut app = app_in(t.path());
    let now = Instant::now();
    app.session.start_edit_new((5, 5));
    type_str(&mut app, "hello", now);
    ctrl_key(&mut app, 'c', now);
    assert!(app.quit);
    app.finish();
    let saved = std::fs::read_to_string(t.path().join("c/demo.clidraw.json")).unwrap();
    assert!(saved.contains("hello"), "{saved}");
}

#[test]
fn colon_wq_saves_and_quits() {
    let t = tempfile::tempdir().unwrap();
    let mut app = app_in(t.path());
    let file = t.path().join("c/demo.clidraw.json");
    let empty = std::fs::read(&file).unwrap();
    insert_shape(&mut app);
    run(&mut app, "wq");
    assert_ne!(std::fs::read(&file).unwrap(), empty);
    assert!(app.quit);
}

#[test]
fn colon_e_opens_or_creates_a_canvas_and_needs_a_name() {
    let t = tempfile::tempdir().unwrap();
    let mut app = app_in(t.path());
    run(&mut app, "e other");
    assert_eq!(app.name, "other");
    assert!(t.path().join("c/other.clidraw.json").exists());
    run(&mut app, "e");
    assert!(app
        .status
        .as_ref()
        .map(|s| s.text.as_str())
        .unwrap()
        .contains("Usage"));
    assert_eq!(app.name, "other");
}

#[test]
fn undo_and_redo_name_the_action_in_the_status_line() {
    let t = tempfile::tempdir().unwrap();
    let mut app = app_in(t.path());
    insert_shape(&mut app);
    let now = Instant::now();
    let ctrl = |c| Event::Key(KeyEvent::new(KeyCode::Char(c), KeyModifiers::CONTROL));
    let status = |app: &App| app.status.as_ref().map(|s| s.text.clone());
    app.on_event(key(KeyCode::Char('u')), now);
    assert_eq!(status(&app).as_deref(), Some("Undid: add 1 element"));
    app.on_event(ctrl('r'), now);
    assert_eq!(status(&app).as_deref(), Some("Redid: add 1 element"));
    run(&mut app, "u");
    assert_eq!(status(&app).as_deref(), Some("Undid: add 1 element"));
    run(&mut app, "redo");
    assert_eq!(status(&app).as_deref(), Some("Redid: add 1 element"));
    run(&mut app, "u");
    app.on_event(key(KeyCode::Char('u')), now);
    assert_eq!(status(&app), None, "nothing to undo");
}

#[test]
fn unknown_command_sets_status_and_esc_cancels_the_prompt() {
    let t = tempfile::tempdir().unwrap();
    let mut app = app_in(t.path());
    run(&mut app, "nope");
    assert_eq!(
        app.status.as_ref().map(|s| s.text.as_str()),
        Some("Unknown command: nope")
    );
    assert!(!app.quit);
    let now = Instant::now();
    app.on_event(key(KeyCode::Char(':')), now);
    assert!(matches!(app.overlay, clidraw::ui::Overlay::Prompt(_)));
    app.on_event(key(KeyCode::Char('q')), now);
    app.on_event(key(KeyCode::Esc), now);
    assert!(matches!(app.overlay, clidraw::ui::Overlay::None));
    assert!(!app.quit);
}

#[test]
fn key_release_does_not_type_into_the_prompt() {
    let t = tempfile::tempdir().unwrap();
    let mut app = vim_app(t.path());
    let now = Instant::now();
    app.on_event(key(KeyCode::Char(':')), now);
    app.on_event(key(KeyCode::Char('q')), now);
    let release = KeyEvent::new_with_kind(
        KeyCode::Char('q'),
        KeyModifiers::NONE,
        KeyEventKind::Release,
    );
    app.on_event(Event::Key(release), now);
    let clidraw::ui::Overlay::Prompt(p) = &app.overlay else {
        panic!("expected a prompt, got another overlay")
    };
    assert_eq!(p.buf(), "q");
}

#[test]
fn only_the_key_of_the_chosen_command_ui_opens_an_overlay() {
    let t = tempfile::tempdir().unwrap();
    let now = Instant::now();
    let mut app = app_in(t.path());
    app.on_event(key(KeyCode::Char(':')), now);
    assert!(matches!(app.overlay, clidraw::ui::Overlay::None));
    ctrl_key(&mut app, 'p', now);
    assert!(matches!(app.overlay, clidraw::ui::Overlay::Actions(_)));
    app.on_event(key(KeyCode::Esc), now);
    app.on_event(key(KeyCode::Char('/')), now);
    assert!(matches!(app.overlay, clidraw::ui::Overlay::Prompt(_)));

    let mut app = app_in(t.path());
    app.session.command_ui = CommandUi::Vim;
    ctrl_key(&mut app, 'p', now);
    assert!(matches!(app.overlay, clidraw::ui::Overlay::None));
    app.on_event(key(KeyCode::Char(':')), now);
    assert!(matches!(app.overlay, clidraw::ui::Overlay::Prompt(_)));
}

#[test]
fn unknown_command_hint_names_ctrl_p_only_in_palette_mode() {
    let t = tempfile::tempdir().unwrap();
    let mut app = app_in(t.path());
    let now = Instant::now();
    ctrl_key(&mut app, 'p', now);
    type_str(&mut app, ":nope", now);
    app.on_event(key(KeyCode::Enter), now);
    assert_eq!(
        app.status.as_ref().map(|s| s.text.as_str()),
        Some("Unknown command: nope (Ctrl-p lists commands)")
    );
    run(&mut app, "nope");
    assert_eq!(
        app.status.as_ref().map(|s| s.text.as_str()),
        Some("Unknown command: nope")
    );
}

fn command_line(app: &mut App, typed: &str, t: Instant) {
    app.session.command_ui = CommandUi::Vim;
    app.on_event(key(KeyCode::Char(':')), t);
    type_str(app, typed, t);
}

#[test]
fn command_line_edits_at_the_caret() {
    let t = tempfile::tempdir().unwrap();
    let mut app = app_in(t.path());
    let now = Instant::now();
    command_line(&mut app, "layoutx", now);
    app.on_event(key(KeyCode::Left), now);
    app.on_event(key(KeyCode::Backspace), now);
    assert_eq!(buffer(&app), "layoux");
    app.on_event(key(KeyCode::Char('t')), now);
    assert_eq!(buffer(&app), "layoutx");
    app.on_event(key(KeyCode::Delete), now);
    assert_eq!(buffer(&app), "layout");
    app.on_event(key(KeyCode::Home), now);
    app.on_event(key(KeyCode::Char('x')), now);
    assert_eq!(buffer(&app), "xlayout");
    app.on_event(key(KeyCode::End), now);
    ctrl_key(&mut app, 'w', now);
    assert_eq!(buffer(&app), "");
    type_str(&mut app, "abc", now);
    app.on_event(key(KeyCode::Left), now);
    ctrl_key(&mut app, 'u', now);
    assert_eq!(buffer(&app), "c");
    app.on_event(key(KeyCode::Esc), now);
    assert!(matches!(app.overlay, clidraw::ui::Overlay::None));
}

#[test]
fn tab_completes_the_verb_and_adds_a_space_only_before_an_argument() {
    let t = tempfile::tempdir().unwrap();
    let mut app = app_in(t.path());
    let now = Instant::now();
    command_line(&mut app, "e", now);
    app.on_event(key(KeyCode::Tab), now);
    assert_eq!(buffer(&app), "e ");
    app.on_event(key(KeyCode::Esc), now);

    command_line(&mut app, "wq", now);
    app.on_event(key(KeyCode::Tab), now);
    assert_eq!(buffer(&app), "wq");
    app.on_event(key(KeyCode::Esc), now);

    command_line(&mut app, "e", now);
    app.on_event(key(KeyCode::Down), now);
    app.on_event(key(KeyCode::Tab), now);
    assert_eq!(buffer(&app), "export ");
}

#[test]
fn find_mode_has_no_completion() {
    let t = tempfile::tempdir().unwrap();
    let mut app = app_in(t.path());
    let now = Instant::now();
    app.on_event(key(KeyCode::Char('/')), now);
    type_str(&mut app, "e", now);
    app.on_event(key(KeyCode::Tab), now);
    assert_eq!(buffer(&app), "/e");
}

#[test]
fn enter_runs_the_typed_text_not_the_highlighted_row() {
    let t = tempfile::tempdir().unwrap();
    let mut app = app_in(t.path());
    let now = Instant::now();
    command_line(&mut app, "e", now);
    app.on_event(key(KeyCode::Down), now);
    app.on_event(key(KeyCode::Enter), now);
    assert!(matches!(app.overlay, clidraw::ui::Overlay::None));
    assert_eq!(
        app.status.as_ref().map(|s| s.text.as_str()),
        Some("Usage: :e <name>")
    );
}

#[test]
fn clicking_a_completion_fills_it_and_clicking_away_closes() {
    let t = tempfile::tempdir().unwrap();
    let mut app = app_in(t.path());
    let now = Instant::now();
    command_line(&mut app, "wq", now);
    click(&mut app, 5, 21, now);
    assert_eq!(buffer(&app), "wq");
    type_str(&mut app, "x", now);
    click(&mut app, 5, 23, now);
    assert_eq!(buffer(&app), "wqx");
    click(&mut app, 5, 2, now);
    assert!(matches!(app.overlay, clidraw::ui::Overlay::None));
}

#[test]
fn clicking_a_completion_replaces_the_typed_prefix() {
    let t = tempfile::tempdir().unwrap();
    let mut app = app_in(t.path());
    let now = Instant::now();
    command_line(&mut app, "lay", now);
    click(&mut app, 5, 21, now);
    assert_eq!(buffer(&app), "layout ");
}

#[test]
fn dialog_prompts_edit_at_the_caret_too() {
    let t = tempfile::tempdir().unwrap();
    let mut app = app_in(t.path());
    let now = Instant::now();
    open_canvases(&mut app, now);
    click(&mut app, 25, 11, now);
    for _ in 0..40 {
        app.on_event(key(KeyCode::Backspace), now);
    }
    type_str(&mut app, "ac", now);
    app.on_event(key(KeyCode::Left), now);
    app.on_event(key(KeyCode::Char('b')), now);
    assert_eq!(buffer(&app), "abc");
}

#[test]
fn colon_meta_sets_reads_back_and_shows_usage() {
    let t = tempfile::tempdir().unwrap();
    let mut app = app_in(t.path());
    insert_shape(&mut app);
    app.session.selection = vec![clidraw::model::Id::from("aaaaaaaa")];
    run(&mut app, "meta");
    assert_eq!(status(&app), "No metadata");
    run(&mut app, "meta url=https://x.test");
    run(&mut app, "meta");
    assert_eq!(status(&app), "url=https://x.test");
    run(&mut app, "meta oops");
    assert_eq!(status(&app), "Usage: :meta <key>=<value>");
    run(&mut app, "meta url=");
    run(&mut app, "meta");
    assert_eq!(status(&app), "No metadata");
}
