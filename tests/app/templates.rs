use super::common::*;
use clidraw::app::App;
use clidraw::model::{Element, End, LineKind, Shape, ShapeKind};
use crossterm::event::KeyCode;
use std::time::Instant;

#[test]
fn palette_inserts_the_flowchart_template_inside_one_frame() {
    let t = tempfile::tempdir().unwrap();
    let mut app = app_in(t.path());
    let now = Instant::now();
    palette_typed(&mut app, "Insert template");
    app.on_event(key(KeyCode::Enter), now);
    assert_eq!(
        palette_labels(&app),
        vec![
            "Flowchart",
            "Architecture dependency",
            "Gantt chart",
            "Sequence diagram",
            "State diagram"
        ]
    );
    app.on_event(key(KeyCode::Enter), now);
    assert!(!palette_open(&app));
    assert_eq!(status(&app), "Inserted template Flowchart");
    assert_inserted_in_one_frame(&mut app);
    app.on_event(key(KeyCode::Char('u')), now);
    assert!(app.session.canvas.elements.is_empty());
}

#[test]
fn palette_spreads_flowchart_template_shapes_apart() {
    let t = tempfile::tempdir().unwrap();
    let mut app = app_in(t.path());
    let now = Instant::now();
    palette_typed(&mut app, "Insert template");
    app.on_event(key(KeyCode::Enter), now);
    app.on_event(key(KeyCode::Enter), now);
    let mut shapes: Vec<&Shape> = app
        .session
        .canvas
        .elements
        .iter()
        .filter_map(|e| match e {
            Element::Shape(s) if s.kind != clidraw::model::ShapeKind::Frame => Some(s),
            _ => None,
        })
        .collect();
    shapes.sort_by_key(|s| s.y);
    let (first, second) = (shapes[0], shapes[1]);
    assert!(second.y - first.y - first.h >= 6);
}

#[test]
fn palette_inserts_the_architecture_template_inside_one_frame() {
    let t = tempfile::tempdir().unwrap();
    let mut app = app_in(t.path());
    let now = Instant::now();
    palette_typed(&mut app, "Insert template");
    app.on_event(key(KeyCode::Enter), now);
    app.on_event(key(KeyCode::Down), now);
    app.on_event(key(KeyCode::Enter), now);
    assert!(!palette_open(&app));
    assert_eq!(status(&app), "Inserted template Architecture dependency");
    assert_inserted_in_one_frame(&mut app);
    app.on_event(key(KeyCode::Char('u')), now);
    assert!(app.session.canvas.elements.is_empty());
}

fn assert_inserted_in_one_frame(app: &mut App) {
    assert_frame_padding(app);
    let frames: Vec<_> = app
        .session
        .canvas
        .elements
        .iter()
        .filter_map(|e| match e {
            Element::Shape(s) if s.kind == clidraw::model::ShapeKind::Frame => Some(s.id.clone()),
            _ => None,
        })
        .collect();
    assert_eq!(frames.len(), 1);
    let mut unit = frames;
    app.session.canvas.expand_frames(&mut unit);
    assert_eq!(unit.len(), app.session.canvas.elements.len());
    assert_eq!(
        app.session.selection.len(),
        app.session.canvas.elements.len()
    );
}

#[test]
fn new_canvas_offers_the_flowchart_template_inside_one_frame() {
    let t = tempfile::tempdir().unwrap();
    let mut app = app_in(t.path());
    let now = Instant::now();
    run_palette(&mut app, "create canvas", now);
    app.on_event(key(KeyCode::Enter), now);
    assert_eq!(
        palette_labels(&app),
        vec![
            "Flowchart",
            "Architecture dependency",
            "Gantt chart",
            "Sequence diagram",
            "State diagram"
        ]
    );
    app.on_event(key(KeyCode::Enter), now);
    assert!(!palette_open(&app));
    assert_eq!(status(&app), "Inserted template Flowchart");
    let frames = app
        .session
        .canvas
        .elements
        .iter()
        .filter(|e| matches!(e, Element::Shape(s) if s.kind == clidraw::model::ShapeKind::Frame))
        .count();
    assert_eq!(frames, 1);
    assert!(!app.session.canvas.elements.is_empty());
}

#[test]
fn new_canvas_stays_blank_when_the_template_list_is_closed() {
    let t = tempfile::tempdir().unwrap();
    let mut app = app_in(t.path());
    let now = Instant::now();
    run_palette(&mut app, "create canvas", now);
    app.on_event(key(KeyCode::Enter), now);
    assert!(palette_open(&app));
    app.on_event(key(KeyCode::Esc), now);
    assert!(!palette_open(&app));
    assert!(app.session.canvas.elements.is_empty());
}

#[test]
fn palette_inserts_the_gantt_template_with_offset_bars_inside_one_frame() {
    let t = tempfile::tempdir().unwrap();
    let mut app = app_in(t.path());
    let now = Instant::now();
    palette_typed(&mut app, "Insert template");
    app.on_event(key(KeyCode::Enter), now);
    type_str(&mut app, "Gantt", now);
    assert_eq!(palette_labels(&app), vec!["Gantt chart"]);
    app.on_event(key(KeyCode::Enter), now);
    assert!(!palette_open(&app));
    assert_eq!(status(&app), "Inserted template Gantt chart");
    let bars: Vec<(i32, i32)> = app
        .session
        .canvas
        .elements
        .iter()
        .filter_map(|e| match e {
            Element::Shape(s) if s.fill.is_some() => Some((s.x, s.w)),
            _ => None,
        })
        .collect();
    let left = bars.iter().map(|b| b.0).min().unwrap();
    let offsets: Vec<i32> = bars.iter().map(|b| b.0 - left).collect();
    let widths: Vec<i32> = bars.iter().map(|b| b.1).collect();
    assert_eq!(offsets, vec![0, 12, 30, 6]);
    assert_eq!(widths, vec![12, 18, 6, 18]);
    let frames: Vec<_> = app
        .session
        .canvas
        .elements
        .iter()
        .filter(|e| matches!(e, Element::Shape(s) if s.kind == clidraw::model::ShapeKind::Frame))
        .collect();
    assert_eq!(frames.len(), 1);
}

#[test]
fn palette_inserts_the_sequence_diagram_template_inside_one_frame() {
    let t = tempfile::tempdir().unwrap();
    let mut app = app_in(t.path());
    let now = Instant::now();
    palette_typed(&mut app, "Insert template");
    app.on_event(key(KeyCode::Enter), now);
    type_str(&mut app, "Sequence", now);
    assert_eq!(palette_labels(&app), vec!["Sequence diagram"]);
    app.on_event(key(KeyCode::Enter), now);
    assert!(!palette_open(&app));
    assert_eq!(status(&app), "Inserted template Sequence diagram");
    let shapes: Vec<&str> = app
        .session
        .canvas
        .elements
        .iter()
        .filter_map(|e| match e {
            Element::Shape(s) => Some(s.label.as_str()),
            _ => None,
        })
        .collect();
    assert_eq!(
        shapes,
        vec!["Sequence diagram", "Client", "Server", "Database"]
    );
    let messages: Vec<&str> = app
        .session
        .canvas
        .elements
        .iter()
        .filter_map(|e| match e {
            Element::Line(l) if !l.label.is_empty() => Some(l.label.as_str()),
            _ => None,
        })
        .collect();
    assert_eq!(messages, vec!["request", "query", "result", "response"]);
    app.on_event(key(KeyCode::Char('u')), now);
    assert!(app.session.canvas.elements.is_empty());
}

#[test]
fn palette_gantt_template_leaves_two_empty_task_rows_and_weeks() {
    let t = tempfile::tempdir().unwrap();
    let mut app = app_in(t.path());
    let now = Instant::now();
    palette_typed(&mut app, "Insert template");
    app.on_event(key(KeyCode::Enter), now);
    type_str(&mut app, "Gantt", now);
    app.on_event(key(KeyCode::Enter), now);
    let rects: Vec<&Shape> = app
        .session
        .canvas
        .elements
        .iter()
        .filter_map(|e| match e {
            Element::Shape(s) if s.kind == ShapeKind::Rect => Some(s),
            _ => None,
        })
        .collect();
    let left = rects.iter().map(|s| s.x).min().unwrap();
    let top = rects.iter().map(|s| s.y).min().unwrap();
    let rows = rects.iter().filter(|s| s.x == left).count();
    let weeks = rects.iter().filter(|s| s.y == top && s.x != left).count();
    assert_eq!(rows, 7);
    assert_eq!(weeks, 8);
}

#[test]
fn palette_sequence_template_spaces_messages_and_extends_lifelines() {
    let t = tempfile::tempdir().unwrap();
    let mut app = app_in(t.path());
    let now = Instant::now();
    palette_typed(&mut app, "Insert template");
    app.on_event(key(KeyCode::Enter), now);
    type_str(&mut app, "Sequence", now);
    app.on_event(key(KeyCode::Enter), now);
    let message_ys: Vec<i32> = app
        .session
        .canvas
        .elements
        .iter()
        .filter_map(|e| match e {
            Element::Line(l) if !l.label.is_empty() => match l.from {
                End::Free(_, y) => Some(y),
                _ => None,
            },
            _ => None,
        })
        .collect();
    assert_eq!(message_ys, vec![9, 13, 17, 21]);
    let lifeline_ends: Vec<i32> = app
        .session
        .canvas
        .elements
        .iter()
        .filter_map(|e| match e {
            Element::Line(l) if l.kind == LineKind::Line => match l.to {
                End::Free(_, y) => Some(y),
                _ => None,
            },
            _ => None,
        })
        .collect();
    assert_eq!(lifeline_ends, vec![37, 37, 37]);
}

#[test]
fn palette_inserts_the_state_diagram_template_inside_one_frame() {
    let t = tempfile::tempdir().unwrap();
    let mut app = app_in(t.path());
    let now = Instant::now();
    palette_typed(&mut app, "Insert template");
    app.on_event(key(KeyCode::Enter), now);
    type_str(&mut app, "State", now);
    assert_eq!(palette_labels(&app), vec!["State diagram"]);
    app.on_event(key(KeyCode::Enter), now);
    assert!(!palette_open(&app));
    assert_eq!(status(&app), "Inserted template State diagram");
    let labels: Vec<&str> = app
        .session
        .canvas
        .elements
        .iter()
        .filter_map(|e| match e {
            Element::Line(l) => Some(l.label.as_str()),
            _ => None,
        })
        .collect();
    assert_eq!(labels, vec!["", "submit", "done", "fail", ""]);
    let states: Vec<&str> = app
        .session
        .canvas
        .elements
        .iter()
        .filter_map(|e| match e {
            Element::Shape(s) if s.kind != clidraw::model::ShapeKind::Frame => {
                Some(s.label.as_str())
            }
            _ => None,
        })
        .collect();
    assert_eq!(states, vec!["Start", "Idle", "Running", "Done", "End"]);
    let frames = app
        .session
        .canvas
        .elements
        .iter()
        .filter(|e| matches!(e, Element::Shape(s) if s.kind == clidraw::model::ShapeKind::Frame))
        .count();
    assert_eq!(frames, 1);
}
