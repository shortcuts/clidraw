use super::grid::Grid;
use super::style::ACTIVE;
use std::collections::VecDeque;
use std::time::Duration;
use unicode_width::UnicodeWidthStr;

const WINDOW: usize = 120;

#[derive(Default)]
pub struct Perf {
    frames: VecDeque<Duration>,
    /// Events handled since the last recorded frame.
    pub events: usize,
    last_events: usize,
    last_bytes: usize,
}

impl Perf {
    pub fn record_frame(&mut self, took: Duration, bytes: usize) {
        if self.frames.len() == WINDOW {
            self.frames.pop_front();
        }
        self.frames.push_back(took);
        self.last_events = std::mem::take(&mut self.events);
        self.last_bytes = bytes;
    }

    pub(super) fn draw(&self, grid: &mut Grid, elements: usize, y: i32) {
        let mut sorted: Vec<f64> = self
            .frames
            .iter()
            .map(|d| d.as_secs_f64() * 1000.0)
            .collect();
        sorted.sort_by(f64::total_cmp);
        let ms = |v: Option<&f64>| v.map_or("-".into(), |v| format!("{v:.1}"));
        let text = format!(
            " frame {}/{}ms  ev {}  {}B  {} el ",
            ms(sorted.get(sorted.len() / 2)),
            ms(sorted.last()),
            self.last_events,
            self.last_bytes,
            elements
        );
        let x = (grid.w - text.width() as i32).max(0);
        grid.text(x, y, &text, ACTIVE);
    }
}
