use super::common::*;
use clidraw::app::App;
use crossterm::event::KeyCode;
use std::time::Instant;

fn help_page(app: &App) -> usize {
    match app.overlay {
        clidraw::ui::Overlay::Help(p, _) => p,
        _ => panic!("help is not open"),
    }
}

#[test]
fn question_mark_opens_help_and_right_left_page_clamped() {
    let t = tempfile::tempdir().unwrap();
    let mut app = app_in(t.path());
    let now = Instant::now();
    app.on_event(key(KeyCode::Char('?')), now);
    assert_eq!(help_page(&app), 0);
    app.on_event(key(KeyCode::Left), now);
    assert_eq!(help_page(&app), 0);
    for _ in 0..3 {
        app.on_event(key(KeyCode::Right), now);
    }
    assert_eq!(help_page(&app), 3);
    app.on_event(key(KeyCode::Left), now);
    assert_eq!(help_page(&app), 2);
    for _ in 0..9 {
        app.on_event(key(KeyCode::Right), now);
    }
    assert_eq!(help_page(&app), 5);
}

#[test]
fn help_closes_on_esc_question_mark_and_outside_click() {
    let t = tempfile::tempdir().unwrap();
    let mut app = app_in(t.path());
    let now = Instant::now();
    for close in [key(KeyCode::Esc), key(KeyCode::Char('?'))] {
        app.on_event(key(KeyCode::Char('?')), now);
        app.on_event(close, now);
        assert!(matches!(app.overlay, clidraw::ui::Overlay::None));
    }
    app.on_event(key(KeyCode::Char('?')), now);
    click(&mut app, 0, 0, now);
    assert!(matches!(app.overlay, clidraw::ui::Overlay::None));
}

#[test]
fn clicking_a_help_tab_selects_its_page() {
    let t = tempfile::tempdir().unwrap();
    let mut app = app_in(t.path());
    let now = Instant::now();
    app.on_event(key(KeyCode::Char('?')), now);
    click(&mut app, 33, 1, now);
    assert_eq!(help_page(&app), 2);
    click(&mut app, 17, 1, now);
    assert_eq!(help_page(&app), 0);
    click(&mut app, 17, 12, now);
    assert_eq!(help_page(&app), 0, "a click off the tabs keeps the modal");
}

fn help_query(app: &App) -> String {
    match &app.overlay {
        clidraw::ui::Overlay::Help(_, q) => q.clone(),
        _ => panic!("help is not open"),
    }
}

#[test]
fn help_typing_builds_the_query_and_backspace_pops() {
    let t = tempfile::tempdir().unwrap();
    let mut app = app_in(t.path());
    let now = Instant::now();
    app.on_event(key(KeyCode::Char('?')), now);
    type_str(&mut app, "d?l", now);
    assert_eq!(help_query(&app), "d?l");
    app.on_event(key(KeyCode::Backspace), now);
    assert_eq!(help_query(&app), "d?");
    app.on_event(key(KeyCode::Right), now);
    assert_eq!(help_page(&app), 0);
}

#[test]
fn help_esc_clears_the_query_then_closes() {
    let t = tempfile::tempdir().unwrap();
    let mut app = app_in(t.path());
    let now = Instant::now();
    app.on_event(key(KeyCode::Char('?')), now);
    type_str(&mut app, "del", now);
    app.on_event(key(KeyCode::Esc), now);
    assert_eq!(help_query(&app), "");
    app.on_event(key(KeyCode::Esc), now);
    assert!(matches!(app.overlay, clidraw::ui::Overlay::None));
}

#[test]
fn help_tab_click_clears_the_query() {
    let t = tempfile::tempdir().unwrap();
    let mut app = app_in(t.path());
    let now = Instant::now();
    app.on_event(key(KeyCode::Char('?')), now);
    type_str(&mut app, "del", now);
    click(&mut app, 33, 1, now);
    assert_eq!(help_query(&app), "");
}

#[test]
fn help_search_matches_substring_fuzzy_and_synonyms() {
    let keys = |q: &str| -> Vec<String> {
        clidraw::ui::help::search(q)
            .into_iter()
            .map(|r| r.0)
            .collect()
    };
    assert_eq!(keys("delete a char"), ["Backspace, Del"]);
    assert!(keys("dlt").contains(&"Del".to_string()));
    assert!(keys("remove").contains(&"Del".to_string()));
    assert!(keys("zzzz").is_empty());
    assert!(clidraw::ui::help::search("e").len() <= 18);
}
