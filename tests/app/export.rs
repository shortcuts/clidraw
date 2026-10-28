use super::common::*;
use clidraw::app::App;
use clidraw::config::Config;
use clidraw::model::{Command, Element, Id, Text};
use clidraw::store::Store;
use crossterm::event::{KeyCode, MouseButton, MouseEventKind};
use std::time::Instant;

#[test]
fn exports_only_the_dragged_area() {
    let t = tempfile::tempdir().unwrap();
    let (mut app, now) = area_app(t.path());
    app.on_event(mouse(MouseEventKind::Down(MouseButton::Left), 0, 2), now);
    app.on_event(mouse(MouseEventKind::Drag(MouseButton::Left), 40, 8), now);
    assert!(app.session.band.is_some());
    app.on_event(mouse(MouseEventKind::Up(MouseButton::Left), 40, 8), now);
    assert!(app.session.band.is_none());
    let out = t.path().join("area.svg");
    type_path(&mut app, out.to_str().unwrap(), now);
    let svg = std::fs::read_to_string(out).unwrap();
    assert!(svg.contains("Alpha") && svg.contains("Beta"), "{svg}");
    assert!(!svg.contains("Gamma"), "{svg}");
}

#[test]
fn exports_only_the_selection() {
    let t = tempfile::tempdir().unwrap();
    let (mut app, now) = area_app(t.path());
    app.on_event(key(KeyCode::Esc), now);
    app.session.selection = vec![Id::from("bbbbbbbb")];
    open_export(&mut app, now);
    click(&mut app, 34, 12, now);
    let out = t.path().join("sel.svg");
    type_path(&mut app, out.to_str().unwrap(), now);
    let svg = std::fs::read_to_string(out).unwrap();
    assert!(svg.contains("Beta"), "{svg}");
    assert!(!svg.contains("Alpha") && !svg.contains("Gamma"), "{svg}");
}

#[test]
fn exports_the_selection_as_a_loadable_clidraw_file() {
    let t = tempfile::tempdir().unwrap();
    let (mut app, now) = area_app(t.path());
    app.on_event(key(KeyCode::Esc), now);
    app.session.selection = vec![Id::from("bbbbbbbb")];
    run(&mut app, "export clidraw");
    click(&mut app, 34, 12, now);
    let out = t.path().join("sel.clidraw.json");
    type_path(&mut app, out.to_str().unwrap(), now);
    let text = std::fs::read_to_string(out).unwrap();
    let canvas: clidraw::model::Canvas = serde_json::from_str(&text).unwrap();
    assert_eq!(canvas.elements.len(), 1);
    assert!(text.contains("Beta") && !text.contains("Alpha"), "{text}");
}

#[test]
fn an_empty_selection_opens_no_prompt() {
    let t = tempfile::tempdir().unwrap();
    let (mut app, now) = area_app(t.path());
    app.on_event(key(KeyCode::Esc), now);
    open_export(&mut app, now);
    click(&mut app, 34, 12, now);
    assert!(matches!(app.overlay, clidraw::ui::Overlay::None));
    assert_eq!(
        app.status.as_ref().map(|s| s.text.as_str()),
        Some("Nothing selected: select a shape first")
    );
}

#[test]
fn export_padding_comes_from_the_config() {
    let t = tempfile::tempdir().unwrap();
    let store = Store::at(t.path().join("c"), t.path().join("state"));
    let cfg = Config {
        export_padding: 0,
        ..Config::default()
    };
    let mut app = App::open_with(store, Some("demo".into()), (80, 24), cfg, vec![]).unwrap();
    app.session.exec(Command::Insert(
        0,
        Element::Text(Text {
            id: Id::from("aaaaaaaa"),
            x: 0,
            y: 0,
            text: "hi".into(),
            color: None,
        }),
    ));
    let now = Instant::now();
    open_export(&mut app, now);
    click(&mut app, 34, 10, now);
    let out = t.path().join("pad.svg");
    type_path(&mut app, out.to_str().unwrap(), now);
    let svg = std::fs::read_to_string(out).unwrap();
    assert!(svg.contains("viewBox=\"0 0 "), "{svg}");
}

#[test]
fn export_scope_runs_the_highlighted_row_from_the_keyboard() {
    let t = tempfile::tempdir().unwrap();
    let mut app = app_in(t.path());
    let now = Instant::now();
    open_export(&mut app, now);
    ctrl_key(&mut app, 'n', now);
    app.on_event(key(KeyCode::Down), now);
    app.on_event(key(KeyCode::Enter), now);
    assert_eq!(
        app.status.as_ref().map(|s| s.text.as_str()),
        Some("Nothing selected: select a shape first")
    );
}

#[test]
fn export_command_takes_a_format_or_lists_them() {
    use clidraw::ui::prompt::PromptKind;
    use clidraw::ui::scope::{ExportFormat, Sink};
    use clidraw::ui::Overlay;
    let t = tempfile::tempdir().unwrap();
    let mut app = app_in(t.path());
    let now = Instant::now();
    run(&mut app, "export");
    assert!(matches!(app.overlay, Overlay::Formats(_)));
    app.on_event(key(KeyCode::Down), now);
    app.on_event(key(KeyCode::Enter), now);
    assert!(matches!(
        app.overlay,
        Overlay::Scope(Sink::File(ExportFormat::Mermaid), _)
    ));
    app.on_event(key(KeyCode::Esc), now);
    for (arg, format) in [
        ("svg", ExportFormat::Svg),
        ("mmd", ExportFormat::Mermaid),
        ("clidraw", ExportFormat::Clidraw),
    ] {
        run(&mut app, &format!("export {arg}"));
        assert!(matches!(app.overlay, Overlay::Scope(f, _) if f == Sink::File(format)));
        app.on_event(key(KeyCode::Esc), now);
    }
    run(&mut app, "export svg");
    app.on_event(key(KeyCode::Enter), now);
    assert!(
        matches!(&app.overlay, Overlay::Prompt(p) if matches!(p.kind, PromptKind::ExportSvg(_)))
    );
    app.on_event(key(KeyCode::Esc), now);
    run(&mut app, "export png");
    assert_eq!(
        app.status.as_ref().map(|s| s.text.as_str()),
        Some("Usage: :export [svg|mmd|clidraw]")
    );
}

#[test]
fn export_asks_before_overwriting_an_existing_file() {
    use clidraw::ui::prompt::PromptKind;
    let t = tempfile::tempdir().unwrap();
    let (mut app, now) = area_app(t.path());
    app.on_event(mouse(MouseEventKind::Down(MouseButton::Left), 0, 2), now);
    app.on_event(mouse(MouseEventKind::Up(MouseButton::Left), 40, 8), now);
    let out = t.path().join("keep.svg");
    std::fs::write(&out, "old").unwrap();
    type_path(&mut app, out.to_str().unwrap(), now);
    assert!(matches!(
        prompt_kind(&app),
        Some(PromptKind::ConfirmOverwrite(..))
    ));
    assert_eq!(std::fs::read_to_string(&out).unwrap(), "old");
    app.on_event(key(KeyCode::Enter), now);
    assert!(std::fs::read_to_string(&out).unwrap().contains("Alpha"));
}

#[test]
fn export_cancelled_overwrite_keeps_the_file() {
    let t = tempfile::tempdir().unwrap();
    let (mut app, now) = area_app(t.path());
    app.on_event(mouse(MouseEventKind::Down(MouseButton::Left), 0, 2), now);
    app.on_event(mouse(MouseEventKind::Up(MouseButton::Left), 40, 8), now);
    let out = t.path().join("keep.svg");
    std::fs::write(&out, "old").unwrap();
    type_path(&mut app, out.to_str().unwrap(), now);
    app.on_event(key(KeyCode::Esc), now);
    assert_eq!(std::fs::read_to_string(&out).unwrap(), "old");
}

// `home_dir` reads USERPROFILE on Windows, not HOME. One test: parallel tests would race on HOME.
#[cfg(unix)]
#[test]
fn a_leading_tilde_expands_in_export_and_import_paths() {
    let t = tempfile::tempdir().unwrap();
    let home = tempfile::tempdir().unwrap();
    std::fs::write(home.path().join("x.mmd"), "flowchart TD\nA --> B\n").unwrap();
    std::env::set_var("HOME", home.path());
    let (mut app, now) = area_app(t.path());
    app.on_event(mouse(MouseEventKind::Down(MouseButton::Left), 0, 2), now);
    app.on_event(mouse(MouseEventKind::Up(MouseButton::Left), 40, 8), now);
    type_path(&mut app, "~/tilde.svg", now);
    assert!(home.path().join("tilde.svg").exists());

    let mut app = app_in(t.path());
    run(&mut app, "import ~/x.mmd");
    assert!(app
        .status
        .as_ref()
        .unwrap()
        .text
        .starts_with("Imported 2 shapes"));
}

#[test]
fn palette_copy_clidraw_queues_the_canvas_as_osc52() {
    let t = tempfile::tempdir().unwrap();
    let mut app = app_in(t.path());
    let now = Instant::now();
    app.session.exec(shape_at("aaaaaaaa", 20, 5));
    ctrl_key(&mut app, 'p', now);
    type_str(&mut app, "copy clidraw", now);
    assert_eq!(palette_labels(&app)[0], "File: Copy clidraw");
    app.on_event(key(KeyCode::Enter), now);
    let seq = app.take_copy().expect("copy queued");
    assert!(seq.starts_with("\x1b]52;c;ewog") && seq.ends_with("\x1b\\"));
    assert!(app.take_copy().is_none());
}

#[test]
fn palette_copy_share_link_queues_the_mermaid_url_and_shows_it() {
    use clidraw::ui::scope::ExportFormat;
    let t = tempfile::tempdir().unwrap();
    let mut app = app_in(t.path());
    let now = Instant::now();
    app.session.exec(shape_at("aaaaaaaa", 20, 5));
    ctrl_key(&mut app, 'p', now);
    type_str(&mut app, "copy share", now);
    assert_eq!(palette_labels(&app)[0], "File: Copy share link");
    app.on_event(key(KeyCode::Enter), now);
    app.on_event(key(KeyCode::Enter), now);
    let text = app.status.as_ref().unwrap().text.clone();
    let url = text
        .strip_prefix("Copied share link: ")
        .expect("status shows the url");
    let fragment = url
        .strip_prefix("https://clidraw.shrtcts.fr/view.html#")
        .expect("url targets the viewer");
    assert!(!fragment.contains(['+', '/', '=']));
    let (mermaid, _) = clidraw::export::render(
        &app.session.canvas,
        ExportFormat::Mermaid,
        Config::default().export_padding,
    );
    assert_eq!(decode_url_safe(fragment), mermaid);
    let seq = app.take_copy().expect("copy queued");
    assert!(seq.starts_with("\x1b]52;c;") && seq.ends_with("\x1b\\"));
}

#[test]
fn palette_copy_share_link_with_no_selection_queues_nothing() {
    let t = tempfile::tempdir().unwrap();
    let mut app = app_in(t.path());
    let now = Instant::now();
    ctrl_key(&mut app, 'p', now);
    type_str(&mut app, "copy share", now);
    app.on_event(key(KeyCode::Enter), now);
    app.on_event(key(KeyCode::Down), now);
    app.on_event(key(KeyCode::Down), now);
    app.on_event(key(KeyCode::Enter), now);
    assert_eq!(
        app.status.as_ref().unwrap().text,
        "Nothing selected: select a shape first"
    );
    assert!(app.take_copy().is_none());
}

#[test]
fn palette_copy_for_obsidian_queues_a_fenced_mermaid_block() {
    use clidraw::ui::scope::ExportFormat;
    let t = tempfile::tempdir().unwrap();
    let mut app = app_in(t.path());
    let now = Instant::now();
    app.session.exec(shape_at("aaaaaaaa", 20, 5));
    ctrl_key(&mut app, 'p', now);
    type_str(&mut app, "copy for obsidian", now);
    assert_eq!(palette_labels(&app)[0], "File: Copy for Obsidian");
    app.on_event(key(KeyCode::Enter), now);
    app.on_event(key(KeyCode::Enter), now);
    assert_eq!(app.status.as_ref().unwrap().text, "Copied for Obsidian");
    let (mermaid, _) = clidraw::export::render(
        &app.session.canvas,
        ExportFormat::Mermaid,
        Config::default().export_padding,
    );
    let seq = app.take_copy().expect("copy queued");
    let payload = seq
        .strip_prefix("\x1b]52;c;")
        .and_then(|s| s.strip_suffix("\x1b\\"))
        .expect("osc 52 sequence");
    assert_eq!(
        decode_base64(payload),
        format!("```mermaid\n{}\n```\n", mermaid.trim_end())
    );
}

#[test]
fn palette_copy_for_obsidian_fences_past_a_backtick_run_in_a_label() {
    let t = tempfile::tempdir().unwrap();
    let mut app = app_in(t.path());
    let now = Instant::now();
    let Command::Insert(at, Element::Shape(mut shape)) = shape_at("aaaaaaaa", 20, 5) else {
        unreachable!()
    };
    shape.label = "a ``` b".into();
    app.session.exec(Command::Insert(at, Element::Shape(shape)));
    ctrl_key(&mut app, 'p', now);
    type_str(&mut app, "copy for obsidian", now);
    app.on_event(key(KeyCode::Enter), now);
    app.on_event(key(KeyCode::Enter), now);
    let seq = app.take_copy().expect("copy queued");
    let payload = seq
        .strip_prefix("\x1b]52;c;")
        .and_then(|s| s.strip_suffix("\x1b\\"))
        .expect("osc 52 sequence");
    let text = decode_base64(payload);
    assert!(text.starts_with("````mermaid\n"), "{text}");
    assert!(text.ends_with("\n````\n"), "{text}");
}

#[test]
fn palette_copy_for_obsidian_with_no_selection_queues_nothing() {
    let t = tempfile::tempdir().unwrap();
    let mut app = app_in(t.path());
    let now = Instant::now();
    ctrl_key(&mut app, 'p', now);
    type_str(&mut app, "copy for obsidian", now);
    app.on_event(key(KeyCode::Enter), now);
    app.on_event(key(KeyCode::Down), now);
    app.on_event(key(KeyCode::Down), now);
    app.on_event(key(KeyCode::Enter), now);
    assert_eq!(
        app.status.as_ref().unwrap().text,
        "Nothing selected: select a shape first"
    );
    assert!(app.take_copy().is_none());
}

fn decode_base64(payload: &str) -> String {
    const ALPHABET: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let (mut bits, mut n, mut out) = (0u32, 0, Vec::new());
    for c in payload.trim_end_matches('=').bytes() {
        let v = ALPHABET
            .iter()
            .position(|&a| a == c)
            .expect("standard base64") as u32;
        bits = bits << 6 | v;
        n += 6;
        if n >= 8 {
            n -= 8;
            out.push((bits >> n) as u8);
        }
    }
    String::from_utf8(out).unwrap()
}

fn decode_url_safe(fragment: &str) -> String {
    let (mut bits, mut n, mut out) = (0u32, 0, Vec::new());
    for c in fragment.bytes() {
        let v = match c {
            b'A'..=b'Z' => c - b'A',
            b'a'..=b'z' => c - b'a' + 26,
            b'0'..=b'9' => c - b'0' + 52,
            b'-' => 62,
            b'_' => 63,
            _ => panic!("not base64url: {c}"),
        };
        bits = bits << 6 | u32::from(v);
        n += 6;
        if n >= 8 {
            n -= 8;
            out.push((bits >> n) as u8);
        }
    }
    String::from_utf8(out).unwrap()
}
