use super::common::*;
use clidraw::model::Id;
use crossterm::event::KeyCode;
use std::time::{Duration, Instant};

#[test]
fn slash_command_selects_matches_in_turn_and_centers_the_view() {
    let t = tempfile::tempdir().unwrap();
    let mut app = app_in(t.path());
    app.session.exec(labeled("aaaaaaaa", 500, "Auth service"));
    app.session.exec(labeled("bbbbbbbb", 0, "db"));
    app.session.exec(labeled("cccccccc", 900, "auth DB"));
    run(&mut app, "/auth");
    assert_eq!(app.session.selection, vec![Id::from("cccccccc")]);
    run(&mut app, "/");
    assert_eq!(app.session.selection, vec![Id::from("aaaaaaaa")]);
    let t0 = Instant::now();
    app.tick(t0);
    app.tick(t0 + Duration::from_millis(120));
    let (sx, _) = app.session.view.to_screen(506, 0);
    assert_eq!(sx, i32::from(app.session.screen.0) / 2);
    run(&mut app, "/zzz");
    assert!(app.status.is_some());
}

#[test]
fn n_and_capital_n_cycle_matches_with_wrap() {
    let t = tempfile::tempdir().unwrap();
    let mut app = app_in(t.path());
    app.session.exec(labeled("aaaaaaaa", 500, "Auth service"));
    app.session.exec(labeled("bbbbbbbb", 0, "db"));
    app.session.exec(labeled("cccccccc", 900, "auth DB"));
    run(&mut app, "/auth");
    let now = Instant::now();
    let steps = [
        ('n', "aaaaaaaa"),
        ('n', "cccccccc"),
        ('N', "aaaaaaaa"),
        ('N', "cccccccc"),
    ];
    for (c, id) in steps {
        app.on_event(key(KeyCode::Char(c)), now);
        assert_eq!(app.session.selection, vec![Id::from(id)], "after {c}");
    }
}

#[test]
fn n_keeps_a_lone_match_selected_and_reports_a_missing_term() {
    let t = tempfile::tempdir().unwrap();
    let mut app = app_in(t.path());
    app.session.exec(labeled("aaaaaaaa", 500, "Auth service"));
    app.on_event(key(KeyCode::Char('n')), Instant::now());
    assert!(app.status.is_some());
    run(&mut app, "/auth");
    app.on_event(key(KeyCode::Char('n')), Instant::now());
    assert_eq!(app.session.selection, vec![Id::from("aaaaaaaa")]);
    assert!(app.status.is_none());
}
