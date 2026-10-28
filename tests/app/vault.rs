use super::common::*;
use clidraw::app::App;
use clidraw::config::Config;
use clidraw::store::Store;
use crossterm::event::KeyCode;
use std::time::Instant;

fn vault_app(dir: &std::path::Path, vault: &std::path::Path) -> App {
    let store = Store::at(dir.join("c"), dir.join("state"));
    let cfg = Config {
        obsidian_vault: Some(vault.to_path_buf()),
        ..Config::default()
    };
    let mut app = App::open_with(store, Some("demo".into()), (80, 24), cfg, vec![]).unwrap();
    app.session.exec(shape_at("aaaaaaaa", 20, 5));
    app
}

fn fenced_block(app: &App) -> String {
    use clidraw::ui::scope::ExportFormat;
    let (mermaid, _) = clidraw::export::render(
        &app.session.canvas,
        ExportFormat::Mermaid,
        Config::default().export_padding,
    );
    format!("```mermaid\n{}\n```\n", mermaid.trim_end())
}

fn open_note_list(app: &mut App, now: Instant) {
    ctrl_key(app, 'p', now);
    type_str(app, "save to obsidian", now);
    app.on_event(key(KeyCode::Enter), now);
    app.on_event(key(KeyCode::Enter), now);
}

fn notes_labels(app: &App) -> Vec<String> {
    match &app.overlay {
        clidraw::ui::Overlay::Notes(_, n) => {
            n.list.labels().iter().map(|l| l.to_string()).collect()
        }
        _ => panic!("note list is not open"),
    }
}

#[test]
fn palette_save_to_obsidian_note_without_a_vault_explains_the_setting() {
    let t = tempfile::tempdir().unwrap();
    let mut app = app_in(t.path());
    let now = Instant::now();
    ctrl_key(&mut app, 'p', now);
    type_str(&mut app, "save to obsidian", now);
    app.on_event(key(KeyCode::Enter), now);
    assert_eq!(
        app.status.as_ref().unwrap().text,
        "Set obsidian_vault in config.toml to save to a vault"
    );
    assert!(matches!(app.overlay, clidraw::ui::Overlay::None));
}

#[test]
fn palette_save_to_obsidian_note_appends_after_a_blank_line() {
    let t = tempfile::tempdir().unwrap();
    let vault = tempfile::tempdir().unwrap();
    std::fs::write(vault.path().join("a.md"), "# A\n").unwrap();
    std::fs::write(vault.path().join("c.md"), "no newline").unwrap();
    std::fs::create_dir(vault.path().join("sub")).unwrap();
    std::fs::write(vault.path().join("sub/b.md"), "").unwrap();
    std::fs::create_dir(vault.path().join(".obsidian")).unwrap();
    std::fs::write(vault.path().join(".obsidian/x.md"), "").unwrap();
    std::fs::write(vault.path().join("pic.png"), "").unwrap();
    let mut app = vault_app(t.path(), vault.path());
    let block = fenced_block(&app);
    let now = Instant::now();
    open_note_list(&mut app, now);
    assert_eq!(
        notes_labels(&app),
        ["+ New note", "a.md", "c.md", "sub/b.md"]
    );
    app.on_event(key(KeyCode::Down), now);
    app.on_event(key(KeyCode::Enter), now);
    assert_eq!(app.status.as_ref().unwrap().text, "Saved to a.md");
    assert_eq!(
        std::fs::read_to_string(vault.path().join("a.md")).unwrap(),
        format!("# A\n\n{block}")
    );
    open_note_list(&mut app, now);
    for _ in 0..2 {
        app.on_event(key(KeyCode::Down), now);
    }
    app.on_event(key(KeyCode::Enter), now);
    assert_eq!(
        std::fs::read_to_string(vault.path().join("c.md")).unwrap(),
        format!("no newline\n\n{block}")
    );
}

#[test]
fn palette_save_to_obsidian_note_creates_a_new_note_and_confirms_before_appending() {
    let t = tempfile::tempdir().unwrap();
    let vault = tempfile::tempdir().unwrap();
    let mut app = vault_app(t.path(), vault.path());
    let block = fenced_block(&app);
    let now = Instant::now();
    open_note_list(&mut app, now);
    app.on_event(key(KeyCode::Enter), now);
    app.on_event(key(KeyCode::Enter), now);
    assert_eq!(app.status.as_ref().unwrap().text, "Saved to demo.md");
    let note = vault.path().join("demo.md");
    assert_eq!(std::fs::read_to_string(&note).unwrap(), block);
    open_note_list(&mut app, now);
    app.on_event(key(KeyCode::Enter), now);
    app.on_event(key(KeyCode::Enter), now);
    assert!(matches!(
        &app.overlay,
        clidraw::ui::Overlay::Prompt(p)
            if matches!(&p.kind, clidraw::ui::prompt::PromptKind::ConfirmAppend(n, _) if n == "demo.md")
    ));
    assert_eq!(std::fs::read_to_string(&note).unwrap(), block);
    app.on_event(key(KeyCode::Enter), now);
    assert_eq!(
        std::fs::read_to_string(&note).unwrap(),
        format!("{block}\n{block}")
    );
}

#[test]
fn palette_save_to_obsidian_note_with_no_selection_writes_nothing() {
    let t = tempfile::tempdir().unwrap();
    let vault = tempfile::tempdir().unwrap();
    let mut app = vault_app(t.path(), vault.path());
    let now = Instant::now();
    ctrl_key(&mut app, 'p', now);
    type_str(&mut app, "save to obsidian", now);
    app.on_event(key(KeyCode::Enter), now);
    app.on_event(key(KeyCode::Down), now);
    app.on_event(key(KeyCode::Down), now);
    app.on_event(key(KeyCode::Enter), now);
    assert_eq!(
        app.status.as_ref().unwrap().text,
        "Nothing selected: select a shape first"
    );
    assert_eq!(std::fs::read_dir(vault.path()).unwrap().count(), 0);
}
