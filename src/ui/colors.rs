use super::palette::{self, Choice};
use super::swatches::Swatches;
use crate::input::Session;

pub fn picker(c: Choice, s: &Session) -> Swatches {
    let title = if c == Choice::Fill { "Fill" } else { "Ink" };
    let mut paints = palette::selected(s)
        .into_iter()
        .filter_map(|e| palette::paint(e, c));
    let first = paints.next();
    let current = first.filter(|_| paints.all(|p| Some(p) == first));
    Swatches::new(title, current, s.screen.0)
}
