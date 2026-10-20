use clidraw::app::App;
use clidraw::input::handles;
use clidraw::model::{Element, End, Id, Line, LineKind, Shape, ShapeKind};
use clidraw::store::Store;
use crossterm::event::{
    Event, KeyCode, KeyEvent, KeyModifiers, MouseButton, MouseEvent, MouseEventKind,
};
use ratatui::backend::TestBackend;
use ratatui::Terminal;
use std::time::{Duration, Instant};

const BUDGET: Duration = Duration::from_millis(16);
const FRAMES: usize = 30;

fn busy_app(dir: &std::path::Path) -> App {
    let store = Store::at(dir.join("c"), dir.join("state"));
    let mut app = App::open(store, Some("busy".into()), (200, 50)).unwrap();
    let id = |i: usize| Id::from(format!("s{i:07}").as_str());
    let els = &mut app.session.canvas.elements;
    for i in 0..2500 {
        els.push(Element::Shape(Shape {
            id: id(i),
            kind: ShapeKind::Rect,
            x: (i % 50) as i32 * 4,
            y: (i / 50) as i32 * 3,
            w: 3,
            h: 3,
            label: String::new(),
        }));
    }
    for i in 0..2499 {
        els.push(Element::Line(Line {
            id: Id::from(format!("l{i:07}").as_str()),
            kind: LineKind::Arrow,
            from: End::Bound(id(i)),
            to: End::Bound(id(i + 1)),
            label: String::new(),
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
    }));
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

fn mouse(kind: MouseEventKind) -> Event {
    mouse_at(kind, 101, 26)
}

fn ctrl(c: char) -> Event {
    Event::Key(KeyEvent::new(KeyCode::Char(c), KeyModifiers::CONTROL))
}

fn median(mut v: Vec<Duration>) -> Duration {
    v.sort();
    v[v.len() / 2]
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
fn band_step_fits_the_budget_on_5000_elements() {
    let t = tempfile::tempdir().unwrap();
    let mut app = busy_app(t.path());
    let mut term = Terminal::new(TestBackend::new(200, 50)).unwrap();
    let now = Instant::now();
    let drag = mouse_at(MouseEventKind::Drag(MouseButton::Left), 150, 40);
    app.on_event(
        mouse_at(MouseEventKind::Down(MouseButton::Left), 250, 0),
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
    let (_, _, (x, y)) = handles(&app.session.canvas, &app.session.selection)
        .pop()
        .unwrap();
    let grab = mouse_at(MouseEventKind::Down(MouseButton::Left), x as u16, y as u16);
    let drag = mouse_at(MouseEventKind::Drag(MouseButton::Left), 150, 40);
    app.on_event(grab, now);
    app.on_event(drag.clone(), now);
    assert!(
        matches!(
            app.session.canvas.get(&line),
            Some(Element::Line(Line {
                to: End::Free(150, 40),
                ..
            }))
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
            app.on_event(ctrl('z'), now);
            frame(&app, &mut term);
            let elapsed = t0.elapsed();
            assert_eq!(app.session.canvas.shape(&target).map(|s| s.x), Some(100));
            app.on_event(ctrl('y'), now);
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
            app.on_event(ctrl('z'), now);
            let t0 = Instant::now();
            app.on_event(ctrl('y'), now);
            frame(&app, &mut term);
            t0.elapsed()
        })
        .collect();
    assert_eq!(app.session.canvas.shape(&target).map(|s| s.x), Some(200));
    let m = median(times);
    assert!(m < BUDGET, "median redo {m:?} exceeds {BUDGET:?}");
}
