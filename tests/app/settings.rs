use super::common::*;
use clidraw::app::App;
use clidraw::config::{CommandUi, DEFAULT_TOML};
use clidraw::ui::theme::Theme;
use clidraw::ui::{Overlay, Severity};
use crossterm::event::KeyCode;
use std::path::Path;
use std::time::Instant;

fn config_text(dir: &Path) -> String {
    std::fs::read_to_string(dir.join("c/config.toml")).unwrap_or_default()
}

fn settings_labels(app: &App) -> Vec<String> {
    match &app.overlay {
        Overlay::Actions(l) => l.labels().iter().map(|s| s.to_string()).collect(),
        _ => panic!("settings list is not open"),
    }
}

/// Opens the settings list from the palette and moves the selection onto `key`.
fn open_on(app: &mut App, name: &str, now: Instant) {
    ctrl_key(app, 'p', now);
    type_str(app, "settings", now);
    app.on_event(key(KeyCode::Enter), now);
    let at = settings_labels(app)
        .iter()
        .position(|l| l.starts_with(&format!("{name} = ")))
        .unwrap();
    for _ in 0..at {
        app.on_event(key(KeyCode::Down), now);
    }
}

#[test]
fn palette_lists_the_settings_with_their_values() {
    let t = tempfile::tempdir().unwrap();
    let mut app = app_in(t.path());
    let now = Instant::now();
    open_on(&mut app, "autosave_ms", now);
    let labels = settings_labels(&app);
    assert_eq!(labels.len(), 14);
    assert_eq!(labels[0], "autosave_ms = 500");
    assert!(labels.contains(&"chrome_accent = unset".to_string()));
}

#[test]
fn enter_toggles_a_flag_and_writes_only_its_line() {
    let t = tempfile::tempdir().unwrap();
    let mut app = app_in(t.path());
    let now = Instant::now();
    open_on(&mut app, "motion", now);
    let before = config_text(t.path());
    app.on_event(key(KeyCode::Enter), now);
    assert!(!app.session.motion);
    assert!(settings_labels(&app).contains(&"motion = false".to_string()));
    let after = config_text(t.path());
    assert!(after.lines().any(|l| l == "motion = false"));
    let kept = |t: &str| {
        t.lines()
            .filter(|l| !l.contains("motion"))
            .collect::<Vec<_>>()
            .join("\n")
    };
    assert_eq!(kept(&before), kept(&after));
}

#[test]
fn enter_cycles_a_choice_and_wraps() {
    let t = tempfile::tempdir().unwrap();
    let mut app = app_in(t.path());
    let now = Instant::now();
    open_on(&mut app, "command_ui", now);
    app.on_event(key(KeyCode::Enter), now);
    assert_eq!(app.session.command_ui, CommandUi::Vim);
    app.on_event(key(KeyCode::Enter), now);
    assert_eq!(app.session.command_ui, CommandUi::Palette);
}

#[test]
fn typed_number_applies_and_shows_in_the_list() {
    let t = tempfile::tempdir().unwrap();
    let mut app = app_in(t.path());
    let now = Instant::now();
    open_on(&mut app, "grid", now);
    app.on_event(key(KeyCode::Enter), now);
    assert!(matches!(app.overlay, Overlay::Prompt(_)));
    app.on_event(key(KeyCode::Backspace), now);
    type_str(&mut app, "4", now);
    app.on_event(key(KeyCode::Enter), now);
    assert_eq!(app.session.grid, 4);
    assert!(settings_labels(&app).contains(&"grid = 4".to_string()));
    assert!(config_text(t.path()).lines().any(|l| l == "grid = 4"));
}

#[test]
fn invalid_number_reports_and_changes_nothing() {
    let t = tempfile::tempdir().unwrap();
    let mut app = app_in(t.path());
    let now = Instant::now();
    open_on(&mut app, "grid", now);
    let before = config_text(t.path());
    app.on_event(key(KeyCode::Enter), now);
    app.on_event(key(KeyCode::Backspace), now);
    type_str(&mut app, "0", now);
    app.on_event(key(KeyCode::Enter), now);
    let status = app.status.as_ref().unwrap();
    assert_eq!(status.severity, Severity::Error);
    assert_eq!(status.text, "grid: expected an integer from 1 to 1000");
    assert_eq!(app.session.grid, 1);
    assert_eq!(config_text(t.path()), before);
    assert!(matches!(app.overlay, Overlay::Actions(_)));
}

#[test]
fn text_setting_is_saved_quoted() {
    let t = tempfile::tempdir().unwrap();
    let vault = tempfile::tempdir().unwrap();
    let mut app = app_in(t.path());
    let now = Instant::now();
    open_on(&mut app, "obsidian_vault", now);
    app.on_event(key(KeyCode::Enter), now);
    let path = vault.path().display().to_string();
    type_str(&mut app, &path, now);
    app.on_event(key(KeyCode::Enter), now);
    assert!(config_text(t.path()).contains(&format!("obsidian_vault = \"{path}\"")));
    assert!(settings_labels(&app).contains(&format!("obsidian_vault = {path}")));
}

#[test]
fn theme_dark_applies_and_auto_asks_for_a_restart() {
    let t = tempfile::tempdir().unwrap();
    let mut app = app_in(t.path());
    let now = Instant::now();
    open_on(&mut app, "theme", now);
    app.on_event(key(KeyCode::Enter), now);
    assert_eq!(app.session.theme, Theme::Dark);
    app.on_event(key(KeyCode::Enter), now);
    app.on_event(key(KeyCode::Enter), now);
    assert_eq!(
        app.status.as_ref().unwrap().text,
        "Restart clidraw to detect the terminal theme"
    );
    assert!(config_text(t.path())
        .lines()
        .any(|l| l == "theme = \"auto\""));
}

#[test]
fn escape_closes_the_list_and_writes_nothing() {
    let t = tempfile::tempdir().unwrap();
    let mut app = app_in(t.path());
    let now = Instant::now();
    open_on(&mut app, "motion", now);
    let before = config_text(t.path());
    app.on_event(key(KeyCode::Esc), now);
    assert!(matches!(app.overlay, Overlay::None));
    assert_eq!(config_text(t.path()), before);
}

#[test]
fn reset_default_needs_the_checkbox_then_restores_the_file() {
    let t = tempfile::tempdir().unwrap();
    let mut app = app_in(t.path());
    let now = Instant::now();
    open_on(&mut app, "motion", now);
    app.on_event(key(KeyCode::Enter), now);
    assert!(!app.session.motion);
    app.on_event(key(KeyCode::Esc), now);
    ctrl_key(&mut app, 'p', now);
    type_str(&mut app, "Reset default", now);
    app.on_event(key(KeyCode::Enter), now);
    assert!(matches!(app.overlay, Overlay::Prompt(_)));
    app.on_event(key(KeyCode::Enter), now);
    assert!(matches!(app.overlay, Overlay::Prompt(_)));
    assert_eq!(app.status.as_ref().unwrap().text, "Tick the box to reset");
    assert_ne!(config_text(t.path()), DEFAULT_TOML);
    app.on_event(key(KeyCode::Char(' ')), now);
    app.on_event(key(KeyCode::Enter), now);
    assert!(matches!(app.overlay, Overlay::None));
    assert_eq!(config_text(t.path()), DEFAULT_TOML);
    assert!(app.session.motion);
    assert_eq!(
        app.status.as_ref().unwrap().text,
        "Settings reset to defaults"
    );
}
