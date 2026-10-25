use clidraw::app::App;
use clidraw::config::Config;
use clidraw::diagram::{DiagramKind, Flowchart};
use clidraw::export::svg;
use clidraw::frame_writer::FrameWriter;
use clidraw::input::handles;
use clidraw::model::{Anchor, Element, End, Id, Line, LineKind, Shape, ShapeKind};
use clidraw::store::Store;
use clidraw::ui::Status;
use clidraw::view::Zoom;
use crossterm::event::{
    Event, KeyCode, KeyEvent, KeyModifiers, MouseButton, MouseEvent, MouseEventKind,
};
use ratatui::backend::{CrosstermBackend, TestBackend};
use ratatui::layout::Rect;
use ratatui::{Terminal, TerminalOptions, Viewport};
use std::cell::Cell;
use std::io::Write;
use std::rc::Rc;
use std::time::{Duration, Instant};

const BUDGET: Duration = Duration::from_millis(16);
const DRAG_FRAME_BUDGET: Duration = Duration::from_millis(30);
const FRAMES: usize = 30;
const LOAD_SAVE_BUDGET: Duration = Duration::from_millis(50);
const STARTUP_BUDGET: Duration = Duration::from_millis(50);
const EXPORT_BUDGET: Duration = Duration::from_millis(100);

fn busy_app(dir: &std::path::Path) -> App {
    busy_app_of(dir, 5000)
}

fn busy_app_of(dir: &std::path::Path, elements: usize) -> App {
    let store = Store::at(dir.join("c"), dir.join("state"));
    let mut app = App::open(store, Some("busy".into()), (200, 50)).unwrap();
    let id = |i: usize| Id::from(format!("s{i:07}").as_str());
    let shapes = elements / 2;
    let els = &mut app.session.canvas.elements;
    for i in 0..shapes {
        els.push(Element::Shape(Shape {
            id: id(i),
            kind: ShapeKind::Rect,
            x: (i % 50) as i32 * 4,
            y: (i / 50) as i32 * 3,
            w: 3,
            h: 3,
            label: String::new(),
            stroke: Default::default(),
            border: Default::default(),
            color: None,
            fill: None,
            align: Default::default(),
            bold: false,
            link: None,
        }));
    }
    // A 1-cell gap makes both end cells of a line coincide; widen one gap so the handles differ.
    if let Some(Element::Shape(b)) = els.get_mut(41) {
        b.x += 2;
    }
    for i in 0..shapes - 1 {
        els.push(Element::Line(Line {
            id: Id::from(format!("l{i:07}").as_str()),
            kind: LineKind::Arrow,
            from: End::Bound(id(i), Anchor::Auto),
            to: End::Bound(id(i + 1), Anchor::Auto),
            label: String::new(),
            label_at: None,
            stroke: Default::default(),
            bends: vec![],
            color: None,
        }));
    }
    els.push(Element::Shape(Shape {
        id: Id::from("target"),
        kind: ShapeKind::Rect,
        x: 100,
        y: 25,
        w: 3,
        h: 3,
        label: String::new(),
        stroke: Default::default(),
        border: Default::default(),
        color: None,
        fill: None,
        align: Default::default(),
        bold: false,
        link: None,
    }));
    app
}

/// `busy_app` with every second line ending on a free cell in the gap right of the next shape; every 100th ends under `target`.
fn free_ends_app(dir: &std::path::Path) -> App {
    let mut app = busy_app(dir);
    for e in &mut app.session.canvas.elements {
        if let Element::Line(l) = e {
            let i: usize = l.id.0[1..].parse().unwrap();
            if i.is_multiple_of(100) {
                l.to = End::Free(101, 26);
            } else if i.is_multiple_of(2) {
                l.to = End::Free(((i + 1) % 50) as i32 * 4 + 3, ((i + 1) / 50) as i32 * 3 + 1);
            }
        }
    }
    app
}

/// `busy_app` with every 8th line aimed one shape further, so its plain route runs through the shape between.
fn blocked_app(dir: &std::path::Path) -> App {
    let mut app = busy_app(dir);
    let shapes = 2500;
    for e in &mut app.session.canvas.elements {
        if let Element::Line(l) = e {
            if let End::Bound(id, a) = &l.to {
                let i: usize = id.0[1..].parse().unwrap();
                if i.is_multiple_of(8) {
                    let to = Id::from(format!("s{:07}", (i + 1).min(shapes - 1)).as_str());
                    l.to = End::Bound(to, *a);
                }
            }
        }
    }
    app
}

/// `busy_app` with every 16th line aimed about 12 columns right and one row down, so its search box is above the old area cap.
fn wide_blocked_app(dir: &std::path::Path) -> App {
    let mut app = busy_app(dir);
    let shapes = 2500;
    for e in &mut app.session.canvas.elements {
        if let Element::Line(l) = e {
            if let End::Bound(id, a) = &l.to {
                let i: usize = id.0[1..].parse().unwrap();
                if i.is_multiple_of(16) {
                    let to = Id::from(format!("s{:07}", (i + 62).min(shapes - 1)).as_str());
                    l.to = End::Bound(to, *a);
                }
            }
        }
    }
    app
}

/// `busy_app` with every 8th line aimed at the shape of the line before it, so their plain routes share segments.
fn overlapped_app(dir: &std::path::Path) -> App {
    let mut app = busy_app(dir);
    let mut prev = None;
    for e in &mut app.session.canvas.elements {
        if let Element::Line(l) = e {
            if let End::Bound(id, a) = l.to.clone() {
                let i: usize = id.0[1..].parse().unwrap();
                if i.is_multiple_of(8) {
                    if let Some(to) = prev.clone() {
                        l.to = End::Bound(to, a);
                    }
                }
                prev = Some(id);
            }
        }
    }
    app
}

const CROSSING_PAN: i32 = -12;

/// `busy_app` panned to a band of small pluses: each vertical arrow crosses a short horizontal one it can go round.
fn crossing_app(dir: &std::path::Path) -> App {
    let mut app = busy_app(dir);
    app.session.view.oy = CROSSING_PAN;
    let free = |id: String, from: (i32, i32), to: (i32, i32)| {
        Element::Line(Line {
            id: Id::from(id.as_str()),
            kind: LineKind::Arrow,
            from: End::Free(from.0, from.1),
            to: End::Free(to.0, to.1),
            label: String::new(),
            label_at: None,
            stroke: Default::default(),
            bends: vec![],
            color: None,
        })
    };
    for k in 0..25 {
        let x = k * 8;
        let els = &mut app.session.canvas.elements;
        els.push(free(format!("h{k:07}"), (x + 2, -7), (x + 4, -7)));
        els.push(free(format!("v{k:07}"), (x + 3, -10), (x + 3, -4)));
    }
    let canvas = &app.session.canvas;
    let Some(Element::Line(v)) = canvas.elements.last() else {
        unreachable!()
    };
    assert!(
        canvas.route(v).len() > 7,
        "the vertical arrow did not detour"
    );
    app
}

fn mouse_at(kind: MouseEventKind, column: u16, row: u16) -> Event {
    Event::Mouse(MouseEvent {
        kind,
        column,
        row,
        modifiers: KeyModifiers::NONE,
    })
}

fn shift_mouse_at(kind: MouseEventKind, column: u16, row: u16) -> Event {
    Event::Mouse(MouseEvent {
        kind,
        column,
        row,
        modifiers: KeyModifiers::SHIFT,
    })
}

fn mouse(kind: MouseEventKind) -> Event {
    mouse_at(kind, 101, 26)
}

fn ctrl(c: char) -> Event {
    Event::Key(KeyEvent::new(KeyCode::Char(c), KeyModifiers::CONTROL))
}

fn plain(c: char) -> Event {
    Event::Key(KeyEvent::new(KeyCode::Char(c), KeyModifiers::NONE))
}

fn median(mut v: Vec<Duration>) -> Duration {
    v.sort();
    let slack = std::env::var("CLIDRAW_PERF_SLACK")
        .ok()
        .and_then(|s| s.parse::<u32>().ok())
        .filter(|n| *n > 0)
        .unwrap_or(1);
    v[v.len() / 2] / slack
}

fn frame(app: &App, term: &mut Terminal<TestBackend>) {
    term.draw(|f| {
        let area = f.area();
        app.render(f.buffer_mut(), area)
    })
    .unwrap();
}

#[test]
#[ignore = "needs a release build: run `make perf`"]
fn frame_fits_the_budget_on_5000_elements() {
    let t = tempfile::tempdir().unwrap();
    let app = busy_app(t.path());
    let mut term = Terminal::new(TestBackend::new(200, 50)).unwrap();
    let times = (0..FRAMES)
        .map(|_| {
            let t0 = Instant::now();
            frame(&app, &mut term);
            t0.elapsed()
        })
        .collect();
    let m = median(times);
    assert!(m < BUDGET, "median frame {m:?} exceeds {BUDGET:?}");
}

#[test]
#[ignore = "needs a release build: run `make perf`"]
fn minimap_frame_fits_the_budget_on_5000_elements() {
    let t = tempfile::tempdir().unwrap();
    let mut app = busy_app(t.path());
    app.overlay = clidraw::ui::Overlay::Map(clidraw::ui::minimap::Target::Canvas);
    let mut term = Terminal::new(TestBackend::new(200, 50)).unwrap();
    let times = (0..FRAMES)
        .map(|_| {
            let t0 = Instant::now();
            frame(&app, &mut term);
            t0.elapsed()
        })
        .collect();
    let m = median(times);
    assert!(m < BUDGET, "median frame {m:?} exceeds {BUDGET:?}");
}

#[test]
#[ignore = "needs a release build: run `make perf`"]
fn overview_frame_fits_the_budget_on_50000_elements() {
    use clidraw::ui::minimap::Target;
    let t = tempfile::tempdir().unwrap();
    let mut app = busy_app_of(t.path(), 50000);
    app.session.selection = app
        .session
        .canvas
        .elements
        .iter()
        .take(100)
        .map(|e| e.id().clone())
        .collect();
    let mut term = Terminal::new(TestBackend::new(200, 50)).unwrap();
    for target in [Target::Selection, Target::Frame, Target::Canvas] {
        app.overlay = clidraw::ui::Overlay::Map(target);
        let times = (0..FRAMES)
            .map(|_| {
                let t0 = Instant::now();
                frame(&app, &mut term);
                t0.elapsed()
            })
            .collect();
        let m = median(times);
        assert!(
            m < BUDGET,
            "{target:?} overview frame {m:?} exceeds {BUDGET:?}"
        );
    }
}

#[test]
#[ignore = "needs a release build: run `make perf`"]
fn frame_fits_the_budget_at_quarter_zoom_on_5000_elements() {
    let t = tempfile::tempdir().unwrap();
    let mut app = busy_app(t.path());
    app.session.view.zoom = Zoom::Quarter;
    let mut term = Terminal::new(TestBackend::new(200, 50)).unwrap();
    let times = (0..FRAMES)
        .map(|_| {
            let t0 = Instant::now();
            frame(&app, &mut term);
            t0.elapsed()
        })
        .collect();
    let m = median(times);
    assert!(m < BUDGET, "median frame {m:?} exceeds {BUDGET:?}");
}

#[test]
#[ignore = "needs a release build: run `make perf`"]
fn frame_fits_the_budget_at_triple_zoom_on_5000_elements() {
    let t = tempfile::tempdir().unwrap();
    let mut app = busy_app(t.path());
    app.session.view.zoom = Zoom::Three;
    let mut term = Terminal::new(TestBackend::new(200, 50)).unwrap();
    let times = (0..FRAMES)
        .map(|_| {
            let t0 = Instant::now();
            frame(&app, &mut term);
            t0.elapsed()
        })
        .collect();
    let m = median(times);
    assert!(m < BUDGET, "median frame {m:?} exceeds {BUDGET:?}");
}

#[test]
#[ignore = "needs a release build: run `make perf`"]
fn frame_fits_the_budget_on_2500_labelled_shapes() {
    let t = tempfile::tempdir().unwrap();
    let mut app = busy_app_of(t.path(), 2);
    app.session.canvas.elements.clear();
    for i in 0..2500 {
        app.session.canvas.elements.push(Element::Shape(Shape {
            id: Id::from(format!("s{i:07}").as_str()),
            kind: ShapeKind::Rect,
            x: i % 4,
            y: i % 40,
            w: 100,
            h: 3,
            label: "x".repeat(80),
            stroke: Default::default(),
            border: Default::default(),
            color: None,
            fill: None,
            align: Default::default(),
            bold: false,
            link: None,
        }));
    }
    let mut term = Terminal::new(TestBackend::new(200, 50)).unwrap();
    let times = (0..FRAMES)
        .map(|_| {
            let t0 = Instant::now();
            frame(&app, &mut term);
            t0.elapsed()
        })
        .collect();
    let m = median(times);
    assert!(m < BUDGET, "median frame {m:?} exceeds {BUDGET:?}");
}

#[test]
#[ignore = "needs a release build: run `make perf`"]
fn drag_step_fits_the_budget_on_5000_elements() {
    let t = tempfile::tempdir().unwrap();
    let mut app = busy_app(t.path());
    let mut term = Terminal::new(TestBackend::new(200, 50)).unwrap();
    let now = Instant::now();
    app.on_event(mouse(MouseEventKind::Down(MouseButton::Left)), now);
    assert!(!app.session.selection.is_empty(), "press missed the shape");
    let times = (0..FRAMES)
        .map(|_| {
            let t0 = Instant::now();
            app.on_event(mouse(MouseEventKind::Drag(MouseButton::Left)), now);
            frame(&app, &mut term);
            t0.elapsed()
        })
        .collect();
    let m = median(times);
    assert!(m < BUDGET, "median drag step {m:?} exceeds {BUDGET:?}");
}

#[test]
#[ignore = "needs a release build: run `make perf`"]
fn frame_fits_the_budget_with_blocked_lines() {
    let t = tempfile::tempdir().unwrap();
    let app = blocked_app(t.path());
    let mut term = Terminal::new(TestBackend::new(200, 50)).unwrap();
    let times = (0..FRAMES)
        .map(|_| {
            let t0 = Instant::now();
            frame(&app, &mut term);
            t0.elapsed()
        })
        .collect();
    let m = median(times);
    assert!(m < BUDGET, "median frame {m:?} exceeds {BUDGET:?}");
}

#[test]
#[ignore = "needs a release build: run `make perf`"]
fn drag_step_fits_the_budget_with_blocked_lines() {
    let t = tempfile::tempdir().unwrap();
    let mut app = blocked_app(t.path());
    let mut term = Terminal::new(TestBackend::new(200, 50)).unwrap();
    let now = Instant::now();
    app.on_event(mouse(MouseEventKind::Down(MouseButton::Left)), now);
    assert!(!app.session.selection.is_empty(), "press missed the shape");
    let times = (0..FRAMES)
        .map(|_| {
            let t0 = Instant::now();
            app.on_event(mouse(MouseEventKind::Drag(MouseButton::Left)), now);
            frame(&app, &mut term);
            t0.elapsed()
        })
        .collect();
    let m = median(times);
    assert!(m < BUDGET, "median drag step {m:?} exceeds {BUDGET:?}");
}

#[test]
#[ignore = "needs a release build: run `make perf`"]
fn frame_fits_the_budget_with_wide_blocked_lines() {
    let t = tempfile::tempdir().unwrap();
    let app = wide_blocked_app(t.path());
    let mut term = Terminal::new(TestBackend::new(200, 50)).unwrap();
    let times = (0..FRAMES)
        .map(|_| {
            let t0 = Instant::now();
            frame(&app, &mut term);
            t0.elapsed()
        })
        .collect();
    let m = median(times);
    assert!(m < BUDGET, "median frame {m:?} exceeds {BUDGET:?}");
}

#[test]
#[ignore = "needs a release build: run `make perf`"]
fn drag_step_fits_the_budget_with_wide_blocked_lines() {
    let t = tempfile::tempdir().unwrap();
    let mut app = wide_blocked_app(t.path());
    let mut term = Terminal::new(TestBackend::new(200, 50)).unwrap();
    let now = Instant::now();
    app.on_event(mouse(MouseEventKind::Down(MouseButton::Left)), now);
    assert!(!app.session.selection.is_empty(), "press missed the shape");
    let times = (0..FRAMES)
        .map(|_| {
            let t0 = Instant::now();
            app.on_event(mouse(MouseEventKind::Drag(MouseButton::Left)), now);
            frame(&app, &mut term);
            t0.elapsed()
        })
        .collect();
    let m = median(times);
    assert!(m < BUDGET, "median drag step {m:?} exceeds {BUDGET:?}");
}

#[test]
#[ignore = "needs a release build: run `make perf`"]
fn frame_fits_the_budget_with_overlapping_lines() {
    let t = tempfile::tempdir().unwrap();
    let app = overlapped_app(t.path());
    let mut term = Terminal::new(TestBackend::new(200, 50)).unwrap();
    let times = (0..FRAMES)
        .map(|_| {
            let t0 = Instant::now();
            frame(&app, &mut term);
            t0.elapsed()
        })
        .collect();
    let m = median(times);
    assert!(m < BUDGET, "median frame {m:?} exceeds {BUDGET:?}");
}

#[test]
#[ignore = "needs a release build: run `make perf`"]
fn drag_step_fits_the_budget_with_overlapping_lines() {
    let t = tempfile::tempdir().unwrap();
    let mut app = overlapped_app(t.path());
    let mut term = Terminal::new(TestBackend::new(200, 50)).unwrap();
    let now = Instant::now();
    app.on_event(mouse(MouseEventKind::Down(MouseButton::Left)), now);
    assert!(!app.session.selection.is_empty(), "press missed the shape");
    let times = (0..FRAMES)
        .map(|_| {
            let t0 = Instant::now();
            app.on_event(mouse(MouseEventKind::Drag(MouseButton::Left)), now);
            frame(&app, &mut term);
            t0.elapsed()
        })
        .collect();
    let m = median(times);
    assert!(m < BUDGET, "median drag step {m:?} exceeds {BUDGET:?}");
}

#[test]
#[ignore = "needs a release build: run `make perf`"]
fn frame_fits_the_budget_with_crossing_lines() {
    let t = tempfile::tempdir().unwrap();
    let app = crossing_app(t.path());
    let mut term = Terminal::new(TestBackend::new(200, 50)).unwrap();
    let times = (0..FRAMES)
        .map(|_| {
            let t0 = Instant::now();
            frame(&app, &mut term);
            t0.elapsed()
        })
        .collect();
    let m = median(times);
    assert!(m < BUDGET, "median frame {m:?} exceeds {BUDGET:?}");
}

#[test]
#[ignore = "needs a release build: run `make perf`"]
fn drag_step_fits_the_budget_with_crossing_lines() {
    let t = tempfile::tempdir().unwrap();
    let mut app = crossing_app(t.path());
    let mut term = Terminal::new(TestBackend::new(200, 50)).unwrap();
    let now = Instant::now();
    let row = (26 - CROSSING_PAN) as u16;
    app.on_event(
        mouse_at(MouseEventKind::Down(MouseButton::Left), 101, row),
        now,
    );
    assert!(!app.session.selection.is_empty(), "press missed the shape");
    let times = (0..FRAMES)
        .map(|_| {
            let t0 = Instant::now();
            app.on_event(
                mouse_at(MouseEventKind::Drag(MouseButton::Left), 101, row),
                now,
            );
            frame(&app, &mut term);
            t0.elapsed()
        })
        .collect();
    let m = median(times);
    assert!(m < BUDGET, "median drag step {m:?} exceeds {BUDGET:?}");
}

#[test]
#[ignore = "needs a release build: run `make perf`"]
fn drag_selection_step_fits_the_budget_on_5000_elements() {
    let t = tempfile::tempdir().unwrap();
    let mut app = busy_app(t.path());
    let mut term = Terminal::new(TestBackend::new(200, 50)).unwrap();
    let m = median(drag_selection_gesture(&mut app, &mut term));
    assert!(
        m < BUDGET,
        "median drag selection step {m:?} exceeds {BUDGET:?}"
    );
}

#[test]
#[ignore = "needs a release build: run `make perf`"]
fn wheel_step_fits_the_budget_on_5000_elements() {
    let t = tempfile::tempdir().unwrap();
    let mut app = busy_app(t.path());
    let mut term = Terminal::new(TestBackend::new(200, 50)).unwrap();
    let now = Instant::now();
    let times = (0..FRAMES)
        .map(|_| {
            let t0 = Instant::now();
            app.on_event(mouse(MouseEventKind::ScrollDown), now);
            frame(&app, &mut term);
            t0.elapsed()
        })
        .collect();
    let m = median(times);
    assert!(m < BUDGET, "median wheel step {m:?} exceeds {BUDGET:?}");
}

#[test]
#[ignore = "needs a release build: run `make perf`"]
fn wheel_step_at_half_zoom_fits_the_budget_on_5000_elements() {
    let t = tempfile::tempdir().unwrap();
    let mut app = busy_app(t.path());
    app.session.view.zoom = Zoom::Half;
    let mut term = Terminal::new(TestBackend::new(200, 50)).unwrap();
    let now = Instant::now();
    let times = (0..FRAMES)
        .map(|_| {
            let t0 = Instant::now();
            app.on_event(mouse(MouseEventKind::ScrollDown), now);
            frame(&app, &mut term);
            t0.elapsed()
        })
        .collect();
    let m = median(times);
    assert!(m < BUDGET, "median wheel step {m:?} exceeds {BUDGET:?}");
}

#[test]
#[ignore = "needs a release build: run `make perf`"]
fn pan_step_fits_the_budget_on_50000_elements() {
    let t = tempfile::tempdir().unwrap();
    let mut app = busy_app_of(t.path(), 50000);
    let mut term = Terminal::new(TestBackend::new(200, 50)).unwrap();
    let m = median(wheel_gesture(&mut app, &mut term));
    assert!(m < BUDGET, "median pan step {m:?} exceeds {BUDGET:?}");
}

#[test]
#[ignore = "needs a release build: run `make perf`"]
fn band_step_fits_the_budget_on_5000_elements() {
    let t = tempfile::tempdir().unwrap();
    let mut app = busy_app(t.path());
    let mut term = Terminal::new(TestBackend::new(200, 50)).unwrap();
    let now = Instant::now();
    let drag = shift_mouse_at(MouseEventKind::Drag(MouseButton::Left), 150, 40);
    app.on_event(
        shift_mouse_at(MouseEventKind::Down(MouseButton::Left), 199, 5),
        now,
    );
    app.on_event(drag.clone(), now);
    assert!(
        app.session.band.is_some(),
        "press hit an element, not empty space"
    );
    let times = (0..FRAMES)
        .map(|_| {
            let t0 = Instant::now();
            app.on_event(drag.clone(), now);
            frame(&app, &mut term);
            t0.elapsed()
        })
        .collect();
    let m = median(times);
    assert!(m < BUDGET, "median band step {m:?} exceeds {BUDGET:?}");
}

#[test]
#[ignore = "needs a release build: run `make perf`"]
fn corner_resize_step_fits_the_budget_on_5000_elements() {
    let t = tempfile::tempdir().unwrap();
    let mut app = busy_app(t.path());
    let mut term = Terminal::new(TestBackend::new(200, 50)).unwrap();
    let now = Instant::now();
    let target = Id::from("target");
    app.session.selection = vec![target.clone()];
    let (_, _, (x, y)) = handles(&app.session.canvas, &app.session.selection)
        .pop()
        .unwrap();
    let grab = mouse_at(MouseEventKind::Down(MouseButton::Left), x as u16, y as u16);
    let drag = mouse_at(MouseEventKind::Drag(MouseButton::Left), 120, 40);
    app.on_event(grab, now);
    app.on_event(drag.clone(), now);
    assert_eq!(
        app.session.canvas.shape(&target).map(|s| s.w),
        Some(21),
        "press missed the corner handle"
    );
    let times = (0..FRAMES)
        .map(|_| {
            let t0 = Instant::now();
            app.on_event(drag.clone(), now);
            frame(&app, &mut term);
            t0.elapsed()
        })
        .collect();
    let m = median(times);
    assert!(m < BUDGET, "median resize step {m:?} exceeds {BUDGET:?}");
}

#[test]
#[ignore = "needs a release build: run `make perf`"]
fn line_end_drag_step_fits_the_budget_on_5000_elements() {
    let t = tempfile::tempdir().unwrap();
    let mut app = busy_app(t.path());
    let mut term = Terminal::new(TestBackend::new(200, 50)).unwrap();
    let now = Instant::now();
    let line = Id::from("l0000040");
    app.session.selection = vec![line.clone()];
    let Some(Element::Line(Line { to: before, .. })) = app.session.canvas.get(&line).cloned()
    else {
        unreachable!()
    };
    let (_, _, (x, y)) = handles(&app.session.canvas, &app.session.selection)
        .pop()
        .unwrap();
    app.session.view.oy = -10;
    let grab = mouse_at(
        MouseEventKind::Down(MouseButton::Left),
        x as u16,
        (y + 10) as u16,
    );
    let drag = shift_mouse_at(MouseEventKind::Drag(MouseButton::Left), 150, 40);
    app.on_event(grab, now);
    app.on_event(drag.clone(), now);
    assert!(
        matches!(
            app.session.canvas.get(&line),
            Some(Element::Line(Line { to, .. })) if *to != before
        ),
        "press missed the line end"
    );
    let times = (0..FRAMES)
        .map(|_| {
            let t0 = Instant::now();
            app.on_event(drag.clone(), now);
            frame(&app, &mut term);
            t0.elapsed()
        })
        .collect();
    let m = median(times);
    assert!(m < BUDGET, "median line end step {m:?} exceeds {BUDGET:?}");
}

#[test]
#[ignore = "needs a release build: run `make perf`"]
fn click_fits_the_budget_on_5000_elements() {
    let t = tempfile::tempdir().unwrap();
    let mut app = busy_app(t.path());
    let mut term = Terminal::new(TestBackend::new(200, 50)).unwrap();
    let now = Instant::now();
    let times = (0..FRAMES)
        .map(|i| {
            let at = now + Duration::from_secs(i as u64);
            let t0 = Instant::now();
            app.on_event(mouse(MouseEventKind::Down(MouseButton::Left)), at);
            app.on_event(mouse(MouseEventKind::Up(MouseButton::Left)), at);
            frame(&app, &mut term);
            t0.elapsed()
        })
        .collect();
    assert_eq!(app.session.selection, vec![Id::from("target")]);
    let m = median(times);
    assert!(m < BUDGET, "median click {m:?} exceeds {BUDGET:?}");
}

fn move_target_in_100_steps(app: &mut App, now: Instant) {
    app.on_event(mouse(MouseEventKind::Down(MouseButton::Left)), now);
    for k in 1..=100 {
        let drag = mouse_at(MouseEventKind::Drag(MouseButton::Left), 101 + k, 26);
        app.on_event(drag, now);
    }
    app.on_event(mouse(MouseEventKind::Up(MouseButton::Left)), now);
}

#[test]
#[ignore = "needs a release build: run `make perf`"]
fn undo_of_a_large_drag_fits_the_budget_on_5000_elements() {
    let t = tempfile::tempdir().unwrap();
    let mut app = busy_app(t.path());
    let mut term = Terminal::new(TestBackend::new(200, 50)).unwrap();
    let now = Instant::now();
    move_target_in_100_steps(&mut app, now);
    let target = Id::from("target");
    let times = (0..FRAMES)
        .map(|_| {
            let t0 = Instant::now();
            app.on_event(plain('u'), now);
            frame(&app, &mut term);
            let elapsed = t0.elapsed();
            assert_eq!(app.session.canvas.shape(&target).map(|s| s.x), Some(100));
            app.on_event(ctrl('r'), now);
            elapsed
        })
        .collect();
    let m = median(times);
    assert!(m < BUDGET, "median undo {m:?} exceeds {BUDGET:?}");
}

#[test]
#[ignore = "needs a release build: run `make perf`"]
fn redo_of_a_large_drag_fits_the_budget_on_5000_elements() {
    let t = tempfile::tempdir().unwrap();
    let mut app = busy_app(t.path());
    let mut term = Terminal::new(TestBackend::new(200, 50)).unwrap();
    let now = Instant::now();
    move_target_in_100_steps(&mut app, now);
    let target = Id::from("target");
    let times = (0..FRAMES)
        .map(|_| {
            app.on_event(plain('u'), now);
            let t0 = Instant::now();
            app.on_event(ctrl('r'), now);
            frame(&app, &mut term);
            t0.elapsed()
        })
        .collect();
    assert_eq!(app.session.canvas.shape(&target).map(|s| s.x), Some(200));
    let m = median(times);
    assert!(m < BUDGET, "median redo {m:?} exceeds {BUDGET:?}");
}

#[test]
#[ignore = "needs a release build: run `make perf`"]
fn save_and_load_fit_the_budget_on_5000_elements() {
    let t = tempfile::tempdir().unwrap();
    let canvas = &busy_app(t.path()).session.canvas;
    let store = Store::at(t.path().join("c"), t.path().join("state"));
    assert!(store.save("busy", canvas).unwrap());
    assert_eq!(store.load("busy").unwrap().elements.len(), 5000);
    let save_times = (0..FRAMES)
        .map(|_| {
            let _ = store.delete("busy");
            let t0 = Instant::now();
            store.save("busy", canvas).unwrap();
            t0.elapsed()
        })
        .collect();
    let load_times = (0..FRAMES)
        .map(|_| {
            let t0 = Instant::now();
            std::hint::black_box(store.load("busy").unwrap());
            t0.elapsed()
        })
        .collect();
    let save = median(save_times);
    let load = median(load_times);
    assert!(
        save < LOAD_SAVE_BUDGET,
        "median save {save:?} exceeds {LOAD_SAVE_BUDGET:?}"
    );
    assert!(
        load < LOAD_SAVE_BUDGET,
        "median load {load:?} exceeds {LOAD_SAVE_BUDGET:?}"
    );
}

#[test]
#[ignore = "needs a release build: run `make perf`"]
fn svg_export_fits_the_budget_on_5000_elements() {
    let t = tempfile::tempdir().unwrap();
    let app = busy_app(t.path());
    assert_eq!(svg(&app.session.canvas).matches("<rect").count(), 2501);
    let times = (0..FRAMES)
        .map(|_| {
            let t0 = Instant::now();
            std::hint::black_box(svg(&app.session.canvas));
            t0.elapsed()
        })
        .collect();
    let m = median(times);
    assert!(
        m < EXPORT_BUDGET,
        "median svg export {m:?} exceeds {EXPORT_BUDGET:?}"
    );
}

#[test]
#[ignore = "needs a release build: run `make perf`"]
fn mermaid_export_fits_the_budget_on_5000_elements() {
    let t = tempfile::tempdir().unwrap();
    let app = busy_app(t.path());
    let out = Flowchart.export(&app.session.canvas);
    assert_eq!((out.skipped, out.text.matches("-->").count()), (0, 2499));
    let times = (0..FRAMES)
        .map(|_| {
            let t0 = Instant::now();
            std::hint::black_box(Flowchart.export(&app.session.canvas));
            t0.elapsed()
        })
        .collect();
    let m = median(times);
    assert!(
        m < EXPORT_BUDGET,
        "median mermaid export {m:?} exceeds {EXPORT_BUDGET:?}"
    );
}

#[test]
#[ignore = "needs a release build: run `make perf`"]
fn area_export_fits_the_budget_on_5000_elements() {
    let t = tempfile::tempdir().unwrap();
    let app = busy_app(t.path());
    let area = (-10, -10, 1000, 1000);
    assert_eq!(app.session.canvas.within(area).elements.len(), 5000);
    let times = (0..FRAMES)
        .map(|_| {
            let t0 = Instant::now();
            std::hint::black_box(app.session.canvas.within(area));
            t0.elapsed()
        })
        .collect();
    let m = median(times);
    assert!(
        m < EXPORT_BUDGET,
        "median area selection {m:?} exceeds {EXPORT_BUDGET:?}"
    );
}

#[test]
#[ignore = "needs a release build: run `make perf`"]
fn selection_clone_fits_the_budget_on_5000_elements() {
    let t = tempfile::tempdir().unwrap();
    let app = busy_app(t.path());
    let canvas = &app.session.canvas;
    let ids: Vec<Id> = canvas.elements.iter().map(|e| e.id().clone()).collect();
    assert_eq!(canvas.clone_canvas(&ids, 2, 1).elements.len(), 5000);
    let times = (0..FRAMES)
        .map(|_| {
            let t0 = Instant::now();
            std::hint::black_box(canvas.clone_canvas(&ids, 2, 1));
            t0.elapsed()
        })
        .collect();
    let m = median(times);
    assert!(
        m < EXPORT_BUDGET,
        "median selection clone {m:?} exceeds {EXPORT_BUDGET:?}"
    );
}

#[test]
#[ignore = "needs a release build: run `make perf`"]
fn startup_fits_the_budget_on_5000_elements() {
    let t = tempfile::tempdir().unwrap();
    let app = busy_app(t.path());
    let store = Store::at(t.path().join("c"), t.path().join("state"));
    store.save("busy", &app.session.canvas).unwrap();
    let times = (0..FRAMES)
        .map(|_| {
            let store = Store::at(t.path().join("c"), t.path().join("state"));
            let t0 = Instant::now();
            let opened = App::open_with(
                store,
                Some("busy".into()),
                (200, 50),
                Config::default(),
                vec![],
            )
            .unwrap();
            let elapsed = t0.elapsed();
            assert_eq!(opened.session.canvas.elements.len(), 5000);
            elapsed
        })
        .collect();
    let m = median(times);
    assert!(
        m < STARTUP_BUDGET,
        "median startup {m:?} exceeds {STARTUP_BUDGET:?}"
    );
}

type Gesture = fn(&mut App, &mut Terminal<TestBackend>) -> Vec<Duration>;

fn repeat_event(app: &mut App, term: &mut Terminal<TestBackend>, event: Event) -> Vec<Duration> {
    let now = Instant::now();
    (0..FRAMES)
        .map(|_| {
            let t0 = Instant::now();
            app.on_event(event.clone(), now);
            frame(app, term);
            t0.elapsed()
        })
        .collect()
}

fn frame_only(app: &mut App, term: &mut Terminal<TestBackend>) -> Vec<Duration> {
    (0..FRAMES)
        .map(|_| {
            let t0 = Instant::now();
            frame(app, term);
            t0.elapsed()
        })
        .collect()
}

fn drag_gesture(app: &mut App, term: &mut Terminal<TestBackend>) -> Vec<Duration> {
    let now = Instant::now();
    app.on_event(mouse(MouseEventKind::Down(MouseButton::Left)), now);
    assert!(!app.session.selection.is_empty(), "press missed the shape");
    repeat_event(app, term, mouse(MouseEventKind::Drag(MouseButton::Left)))
}

fn drag_selection_gesture(app: &mut App, term: &mut Terminal<TestBackend>) -> Vec<Duration> {
    let now = Instant::now();
    let selected = (0..999).map(|i| Id::from(format!("s{i:07}").as_str()));
    app.session.selection = std::iter::once(Id::from("target"))
        .chain(selected)
        .collect();
    app.on_event(mouse(MouseEventKind::Down(MouseButton::Left)), now);
    assert_eq!(
        app.session.selection.len(),
        1000,
        "press dropped the selection"
    );
    (0..FRAMES)
        .map(|i| {
            let drag = mouse_at(
                MouseEventKind::Drag(MouseButton::Left),
                102 + (i % 2) as u16,
                26,
            );
            let t0 = Instant::now();
            app.on_event(drag, now);
            frame(app, term);
            t0.elapsed()
        })
        .collect()
}

fn wheel_gesture(app: &mut App, term: &mut Terminal<TestBackend>) -> Vec<Duration> {
    repeat_event(app, term, mouse(MouseEventKind::ScrollDown))
}

fn band_gesture(app: &mut App, term: &mut Terminal<TestBackend>) -> Vec<Duration> {
    let now = Instant::now();
    app.on_event(
        shift_mouse_at(MouseEventKind::Down(MouseButton::Left), 199, 5),
        now,
    );
    app.on_event(
        shift_mouse_at(MouseEventKind::Drag(MouseButton::Left), 150, 40),
        now,
    );
    assert!(
        app.session.band.is_some(),
        "press hit an element, not empty space"
    );
    repeat_event(
        app,
        term,
        shift_mouse_at(MouseEventKind::Drag(MouseButton::Left), 150, 40),
    )
}

fn resize_gesture(app: &mut App, term: &mut Terminal<TestBackend>) -> Vec<Duration> {
    let now = Instant::now();
    let target = Id::from("target");
    app.session.selection = vec![target.clone()];
    let (_, _, (x, y)) = handles(&app.session.canvas, &app.session.selection)
        .pop()
        .unwrap();
    app.on_event(
        mouse_at(MouseEventKind::Down(MouseButton::Left), x as u16, y as u16),
        now,
    );
    app.on_event(
        mouse_at(MouseEventKind::Drag(MouseButton::Left), 120, 40),
        now,
    );
    assert_eq!(
        app.session.canvas.shape(&target).map(|s| s.w),
        Some(21),
        "press missed the corner handle"
    );
    repeat_event(
        app,
        term,
        mouse_at(MouseEventKind::Drag(MouseButton::Left), 120, 40),
    )
}

fn line_end_gesture(app: &mut App, term: &mut Terminal<TestBackend>) -> Vec<Duration> {
    let now = Instant::now();
    let line = Id::from("l0000040");
    app.session.selection = vec![line.clone()];
    let Some(Element::Line(Line { to: before, .. })) = app.session.canvas.get(&line).cloned()
    else {
        unreachable!()
    };
    let (_, _, (x, y)) = handles(&app.session.canvas, &app.session.selection)
        .pop()
        .unwrap();
    app.session.view.oy = -10;
    app.on_event(
        mouse_at(
            MouseEventKind::Down(MouseButton::Left),
            x as u16,
            (y + 10) as u16,
        ),
        now,
    );
    app.on_event(
        shift_mouse_at(MouseEventKind::Drag(MouseButton::Left), 150, 40),
        now,
    );
    assert!(
        matches!(
            app.session.canvas.get(&line),
            Some(Element::Line(Line { to, .. })) if *to != before
        ),
        "press missed the line end"
    );
    repeat_event(
        app,
        term,
        shift_mouse_at(MouseEventKind::Drag(MouseButton::Left), 150, 40),
    )
}

fn click_gesture(app: &mut App, term: &mut Terminal<TestBackend>) -> Vec<Duration> {
    let now = Instant::now();
    (0..FRAMES)
        .map(|i| {
            let at = now + Duration::from_secs(i as u64);
            let t0 = Instant::now();
            app.on_event(mouse(MouseEventKind::Down(MouseButton::Left)), at);
            app.on_event(mouse(MouseEventKind::Up(MouseButton::Left)), at);
            frame(app, term);
            t0.elapsed()
        })
        .collect()
}

fn undo_gesture(app: &mut App, term: &mut Terminal<TestBackend>) -> Vec<Duration> {
    let now = Instant::now();
    move_target_in_100_steps(app, now);
    (0..FRAMES)
        .map(|_| {
            let t0 = Instant::now();
            app.on_event(plain('u'), now);
            frame(app, term);
            let elapsed = t0.elapsed();
            app.on_event(ctrl('r'), now);
            elapsed
        })
        .collect()
}

fn redo_gesture(app: &mut App, term: &mut Terminal<TestBackend>) -> Vec<Duration> {
    let now = Instant::now();
    move_target_in_100_steps(app, now);
    (0..FRAMES)
        .map(|_| {
            app.on_event(plain('u'), now);
            let t0 = Instant::now();
            app.on_event(ctrl('r'), now);
            frame(app, term);
            t0.elapsed()
        })
        .collect()
}

#[test]
#[ignore = "prints a table, not a gate: run `make perf`"]
fn scaling_table() {
    let gestures: [(&str, Gesture); 10] = [
        ("frame", frame_only),
        ("drag", drag_gesture),
        ("drag sel", drag_selection_gesture),
        ("wheel", wheel_gesture),
        ("band", band_gesture),
        ("resize", resize_gesture),
        ("line end", line_end_gesture),
        ("click", click_gesture),
        ("undo", undo_gesture),
        ("redo", redo_gesture),
    ];
    println!(
        "{:<10} {:>9} {:>12} {:>12}",
        "gesture", "elements", "median", "worst"
    );
    for (name, gesture) in gestures {
        for elements in [500, 5000, 50000] {
            let t = tempfile::tempdir().unwrap();
            let mut app = busy_app_of(t.path(), elements);
            assert_eq!(app.session.canvas.elements.len(), elements);
            let mut term = Terminal::new(TestBackend::new(200, 50)).unwrap();
            let times = gesture(&mut app, &mut term);
            let worst = *times.iter().max().unwrap();
            let m = median(times);
            println!(
                "{name:<10} {elements:>9} {:>12} {:>12}",
                format!("{m:?}"),
                format!("{worst:?}")
            );
        }
    }
}

const ONE_CELL_BYTES: usize = 4 * 1024;

#[derive(Clone, Default)]
struct Counting(Rc<Cell<(usize, usize)>>);

impl Write for Counting {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        let (bytes, writes) = self.0.get();
        self.0.set((bytes + buf.len(), writes + 1));
        Ok(buf.len())
    }

    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}

type Counted = Terminal<CrosstermBackend<FrameWriter<Counting>>>;

fn counted_terminal() -> (Counting, FrameWriter<Counting>, Counted) {
    let counter = Counting::default();
    let out = FrameWriter::new(counter.clone());
    let viewport = Viewport::Fixed(Rect::new(0, 0, 200, 50));
    let term = Terminal::with_options(
        CrosstermBackend::new(out.handle()),
        TerminalOptions { viewport },
    )
    .unwrap();
    (counter, out, term)
}

/// One pass of the loop in `main.rs`. Returns the (bytes, write calls) it sent.
fn send_frame(
    app: &mut App,
    counter: &Counting,
    out: &FrameWriter<Counting>,
    term: &mut Counted,
) -> (usize, usize) {
    let (b0, w0) = counter.0.get();
    if app.dirty {
        write!(term.backend_mut(), "\x1b[?2026h").unwrap();
        term.draw(|f| {
            let area = f.area();
            app.render(f.buffer_mut(), area)
        })
        .unwrap();
        write!(term.backend_mut(), "\x1b[?2026l").unwrap();
        out.send().unwrap();
        app.dirty = false;
    }
    let (b1, w1) = counter.0.get();
    (b1 - b0, w1 - w0)
}

#[test]
fn bytes_per_frame_fit_the_budget_on_5000_elements() {
    let t = tempfile::tempdir().unwrap();
    let mut app = busy_app(t.path());
    let (counter, out, mut term) = counted_terminal();
    let now = Instant::now();

    let first = send_frame(&mut app, &counter, &out, &mut term);

    app.status = Some(Status::notice("a"));
    app.dirty = true;
    send_frame(&mut app, &counter, &out, &mut term);
    app.status = Some(Status::notice("b"));
    app.dirty = true;
    let one_cell = send_frame(&mut app, &counter, &out, &mut term);

    app.on_event(mouse(MouseEventKind::Down(MouseButton::Left)), now);
    send_frame(&mut app, &counter, &out, &mut term);
    app.on_event(mouse(MouseEventKind::Drag(MouseButton::Left)), now);
    let drag = send_frame(&mut app, &counter, &out, &mut term);

    app.session.view.pan(0, 1);
    app.dirty = true;
    let pan = send_frame(&mut app, &counter, &out, &mut term);

    let idle = send_frame(&mut app, &counter, &out, &mut term);

    println!("{:<12} {:>10} {:>12}", "frame", "bytes", "write calls");
    for (name, (bytes, writes)) in [
        ("first", first),
        ("one cell", one_cell),
        ("drag", drag),
        ("pan 1 row", pan),
        ("idle", idle),
    ] {
        println!("{name:<12} {bytes:>10} {writes:>12}");
    }

    assert!(
        one_cell.0 < ONE_CELL_BYTES,
        "one-cell change sent {} bytes, budget is {ONE_CELL_BYTES}",
        one_cell.0
    );
    assert_eq!(idle.0, 0, "idle frame sent bytes");
    for (name, (_, writes)) in [
        ("first", first),
        ("one cell", one_cell),
        ("drag", drag),
        ("pan", pan),
    ] {
        assert_eq!(writes, 1, "{name} frame took {writes} write calls");
    }
}

#[test]
fn take_bytes_counts_writes_and_resets() {
    let mut out = FrameWriter::new(Counting::default());
    out.write_all(b"0123456789").unwrap();
    assert_eq!(out.take_bytes(), 10);
    assert_eq!(out.take_bytes(), 0);
}

#[test]
fn median_divides_by_perf_slack() {
    let samples = || {
        vec![
            Duration::from_millis(40),
            Duration::from_millis(8),
            Duration::from_millis(20),
        ]
    };
    std::env::set_var("CLIDRAW_PERF_SLACK", "4");
    assert_eq!(median(samples()), Duration::from_millis(5));
    std::env::set_var("CLIDRAW_PERF_SLACK", "0");
    assert_eq!(median(samples()), Duration::from_millis(20));
    std::env::remove_var("CLIDRAW_PERF_SLACK");
    assert_eq!(median(samples()), Duration::from_millis(20));
}

#[test]
#[ignore = "needs a release build: run `make perf`"]
fn hover_step_fits_the_budget_on_5000_elements() {
    let t = tempfile::tempdir().unwrap();
    let mut app = busy_app(t.path());
    let mut term = Terminal::new(TestBackend::new(200, 50)).unwrap();
    let now = Instant::now();
    let (on, off) = (
        mouse(MouseEventKind::Moved),
        mouse_at(MouseEventKind::Moved, 199, 49),
    );
    let times = (0..FRAMES)
        .map(|i| {
            let t0 = Instant::now();
            app.on_event(if i % 2 == 0 { on.clone() } else { off.clone() }, now);
            assert!(app.dirty, "hover target did not change");
            frame(&app, &mut term);
            app.dirty = false;
            t0.elapsed()
        })
        .collect();
    let m = median(times);
    assert!(m < BUDGET, "median hover step {m:?} exceeds {BUDGET:?}");
}

#[test]
#[ignore = "needs a release build: run `make perf`"]
fn drag_step_with_free_ends_fits_the_budget_on_5000_elements() {
    let t = tempfile::tempdir().unwrap();
    let mut app = free_ends_app(t.path());
    let now = Instant::now();
    app.on_event(mouse(MouseEventKind::Down(MouseButton::Left)), now);
    assert!(!app.session.selection.is_empty(), "press missed the shape");
    let times = (0..FRAMES)
        .map(|i| {
            let t0 = Instant::now();
            let col = if i % 2 == 0 { 103 } else { 101 };
            app.on_event(
                mouse_at(MouseEventKind::Drag(MouseButton::Left), col, 26),
                now,
            );
            t0.elapsed()
        })
        .collect();
    let m = median(times);
    assert!(m < BUDGET, "median drag step {m:?} exceeds {BUDGET:?}");
}

#[test]
#[ignore = "needs a release build: run `make perf`"]
fn frame_after_a_drag_with_free_ends_fits_the_budget_on_5000_elements() {
    let t = tempfile::tempdir().unwrap();
    let mut app = free_ends_app(t.path());
    let mut term = Terminal::new(TestBackend::new(200, 50)).unwrap();
    let now = Instant::now();
    app.on_event(mouse(MouseEventKind::Down(MouseButton::Left)), now);
    assert!(!app.session.selection.is_empty(), "press missed the shape");
    let times = (0..FRAMES)
        .map(|i| {
            let col = if i % 2 == 0 { 103 } else { 101 };
            app.on_event(
                mouse_at(MouseEventKind::Drag(MouseButton::Left), col, 26),
                now,
            );
            let t0 = Instant::now();
            frame(&app, &mut term);
            t0.elapsed()
        })
        .collect::<Vec<_>>();
    let m = median(times);
    assert!(
        m < DRAG_FRAME_BUDGET,
        "median frame after drag {m:?} exceeds {DRAG_FRAME_BUDGET:?}"
    );
}

#[test]
#[ignore = "needs a release build: run `make perf`"]
fn hover_step_with_a_selected_line_fits_the_budget_on_5000_elements() {
    let t = tempfile::tempdir().unwrap();
    let mut app = busy_app(t.path());
    app.session.selection = vec![Id::from("l0000001")];
    let mut term = Terminal::new(TestBackend::new(200, 50)).unwrap();
    let now = Instant::now();
    let (on, off) = (
        mouse(MouseEventKind::Moved),
        mouse_at(MouseEventKind::Moved, 199, 49),
    );
    let times = (0..FRAMES)
        .map(|i| {
            let t0 = Instant::now();
            app.on_event(if i % 2 == 0 { on.clone() } else { off.clone() }, now);
            assert!(app.dirty, "hover target did not change");
            frame(&app, &mut term);
            app.dirty = false;
            t0.elapsed()
        })
        .collect();
    let m = median(times);
    assert!(m < BUDGET, "median hover step {m:?} exceeds {BUDGET:?}");
}
