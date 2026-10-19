use clidraw::app::App;
use clidraw::model::{Element, End, Id, Line, LineKind, Shape, ShapeKind};
use clidraw::store::Store;
use crossterm::event::{Event, KeyModifiers, MouseButton, MouseEvent, MouseEventKind};
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

fn mouse(kind: MouseEventKind) -> Event {
    Event::Mouse(MouseEvent {
        kind,
        column: 101,
        row: 26,
        modifiers: KeyModifiers::NONE,
    })
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
