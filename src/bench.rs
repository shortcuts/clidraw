use super::{draw_frame, restore};
use anyhow::{bail, Result};
use clidraw::app::App;
use clidraw::frame_writer::FrameWriter;
use clidraw::model::{Anchor, Element, End, Id, Line, LineKind, Shape, ShapeKind};
use clidraw::store::Store;
use crossterm::event::{Event, KeyModifiers, MouseButton, MouseEvent, MouseEventKind};
use crossterm::execute;
use crossterm::terminal::{enable_raw_mode, EnterAlternateScreen};
use ratatui::backend::CrosstermBackend;
use ratatui::Terminal;
use std::io::{stdout, Stdout};
use std::panic::{self, AssertUnwindSafe};
use std::time::{Duration, Instant};

const ELEMENTS: usize = 5000;
const LEG_STEPS: i32 = 30;
const PAN_STEPS: usize = 120;

type Term = Terminal<CrosstermBackend<FrameWriter<Stdout>>>;

fn mouse(kind: MouseEventKind, column: u16, row: u16) -> Event {
    Event::Mouse(MouseEvent {
        kind,
        column,
        row,
        modifiers: KeyModifiers::NONE,
    })
}

// Mirrors busy_app_of in tests/perf.rs; keep the two in step.
fn generated_canvas(elements: usize, target: (i32, i32)) -> Vec<Element> {
    let id = |i: usize| Id::from(format!("s{i:07}").as_str());
    let shape = |id: Id, x: i32, y: i32| {
        Element::Shape(Shape {
            id,
            kind: ShapeKind::Rect,
            x,
            y,
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
        })
    };
    let shapes = elements / 2;
    let mut els = Vec::with_capacity(elements + 1);
    for i in 0..shapes {
        els.push(shape(id(i), (i % 50) as i32 * 4, (i / 50) as i32 * 3));
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
    els.push(shape(Id::from("target"), target.0, target.1));
    els
}

/// A press on the target's left border, 120 drag steps around a rectangle, then a release.
fn script_drag(w: u16, h: u16) -> Vec<Event> {
    let (cx, cy) = (i32::from(w / 2 + 1), i32::from(h / 2 + 1));
    let at = |x: i32, y: i32| {
        // Edge pan starts 2 cells from the border; stay clear of it.
        let (x, y) = (x.clamp(3, i32::from(w) - 4), y.clamp(3, i32::from(h) - 4));
        mouse(MouseEventKind::Drag(MouseButton::Left), x as u16, y as u16)
    };
    let corners = [(16, -6), (16, 6), (-16, 6), (-16, -6)];
    let mut events = vec![mouse(
        MouseEventKind::Down(MouseButton::Left),
        cx as u16 - 1,
        cy as u16,
    )];
    let mut from = (0, 0);
    for to in corners {
        for i in 1..=LEG_STEPS {
            let step = |a: i32, b: i32| a + (b - a) * i / LEG_STEPS;
            events.push(at(cx + step(from.0, to.0), cy + step(from.1, to.1)));
        }
        from = to;
    }
    events.push(mouse(
        MouseEventKind::Up(MouseButton::Left),
        cx as u16,
        cy as u16,
    ));
    events
}

fn script_pan(w: u16, h: u16) -> Vec<Event> {
    vec![mouse(MouseEventKind::ScrollDown, w / 2, h / 2); PAN_STEPS]
}

/// Returns (median, p99, worst) by nearest rank.
fn summarize(v: &mut [Duration]) -> (Duration, Duration, Duration) {
    v.sort();
    let n = v.len();
    (v[n / 2], v[(n * 99 / 100).min(n - 1)], v[n - 1])
}

/// Times each event from input to the flushed frame. Returns the times and the bytes written.
fn replay(
    app: &mut App,
    out: &FrameWriter<Stdout>,
    terminal: &mut Term,
    events: Vec<Event>,
) -> Result<(Vec<Duration>, usize)> {
    let (mut times, mut bytes) = (vec![], 0);
    for ev in events {
        let t0 = Instant::now();
        app.on_events(vec![ev], t0);
        if app.dirty {
            bytes += draw_frame(app, out, terminal)?;
            times.push(t0.elapsed());
        }
    }
    Ok((times, bytes))
}

fn ms(d: Duration) -> String {
    format!("{:.1}", d.as_secs_f64() * 1000.0)
}

fn row(phase: &str, mut times: Vec<Duration>, bytes: usize) -> String {
    let (median, p99, worst) = summarize(&mut times);
    let frames = times.len();
    format!(
        "{phase:<6}{frames:>7}{:>8}{:>8}{:>8}{:>9}{bytes:>9}",
        ms(median),
        ms(p99),
        ms(worst),
        bytes / frames,
    )
}

fn measure(dir: &std::path::Path) -> Result<Vec<String>> {
    let (w, h) = crossterm::terminal::size()?;
    if w < 40 || h < 16 {
        bail!("terminal too small: need 40x16, have {w}x{h}");
    }
    let store = Store::at(dir.join("c"), dir.join("state"));
    let mut app = App::open(store, Some("bench".into()), (w, h))?;
    let target = app
        .session
        .view
        .to_world(i32::from(w / 2), i32::from(h / 2));
    app.session
        .canvas
        .elements
        .extend(generated_canvas(ELEMENTS, target));

    enable_raw_mode()?;
    execute!(stdout(), EnterAlternateScreen)?;
    let out = FrameWriter::new(stdout());
    let mut terminal = Terminal::new(CrosstermBackend::new(out.handle()))?;
    draw_frame(&mut app, &out, &mut terminal)?;

    let mut drag = script_drag(w, h);
    app.on_events(vec![drag.remove(0)], Instant::now());
    if app.session.selection.is_empty() {
        bail!("press missed the target");
    }
    let (times, bytes) = replay(&mut app, &out, &mut terminal, drag)?;
    let drag_row = row("drag", times, bytes);
    let (times, bytes) = replay(&mut app, &out, &mut terminal, script_pan(w, h))?;
    Ok(vec![drag_row, row("pan", times, bytes)])
}

/// Replays a scripted drag and pan on a generated canvas and prints the frame times.
pub fn run() -> Result<()> {
    // The bench never saves, so it works in a scratch store, not the user's canvases.
    let dir = std::env::temp_dir().join(format!("clidraw-bench-{}", std::process::id()));
    let result = panic::catch_unwind(AssertUnwindSafe(|| measure(&dir)));
    restore();
    let _ = std::fs::remove_dir_all(&dir);
    match result {
        Ok(rows) => {
            println!("phase  frames  median     p99   worst  B/frame  total B");
            rows?.iter().for_each(|r| println!("{r}"));
            Ok(())
        }
        Err(payload) => panic::resume_unwind(payload),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn pos(ev: &Event) -> (u16, u16) {
        match ev {
            Event::Mouse(m) => (m.column, m.row),
            _ => panic!("not a mouse event"),
        }
    }

    #[test]
    fn summarize_picks_nearest_rank() {
        let mut v: Vec<Duration> = (1..=120).rev().map(Duration::from_millis).collect();
        let (median, p99, worst) = summarize(&mut v);
        assert_eq!(median, Duration::from_millis(61));
        assert_eq!(p99, Duration::from_millis(119));
        assert_eq!(worst, Duration::from_millis(120));
    }

    #[test]
    fn drag_script_presses_the_target_then_stays_off_the_edge() {
        let ev = script_drag(200, 50);
        assert_eq!(ev.len(), 122);
        assert!(
            matches!(&ev[0], Event::Mouse(m) if m.kind == MouseEventKind::Down(MouseButton::Left))
        );
        assert_eq!(pos(&ev[0]), (100, 26));
        assert!(
            matches!(&ev[121], Event::Mouse(m) if m.kind == MouseEventKind::Up(MouseButton::Left))
        );
        for e in &ev[1..121] {
            assert!(
                matches!(e, Event::Mouse(m) if m.kind == MouseEventKind::Drag(MouseButton::Left))
            );
            let (x, y) = pos(e);
            assert!((3..=196).contains(&x) && (3..=46).contains(&y), "{x},{y}");
        }
        assert_eq!(pos(&ev[1]), (101, 26));
    }

    #[test]
    fn pan_script_scrolls_120_times() {
        let ev = script_pan(200, 50);
        assert_eq!(ev.len(), 120);
        assert!(ev
            .iter()
            .all(|e| matches!(e, Event::Mouse(m) if m.kind == MouseEventKind::ScrollDown)));
    }
}
