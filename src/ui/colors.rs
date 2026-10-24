use super::list::{List, Row};
use super::palette::Choice;
use crate::model::Paint;

pub fn list(c: Choice) -> List<Option<Paint>> {
    let row = |value: Option<Paint>| Row {
        value,
        label: value.map_or("none", Paint::name).to_string(),
        key: String::new(),
    };
    let title = if c == Choice::Fill { "Fill" } else { "Ink" };
    let rows = std::iter::once(None)
        .chain(Paint::ALL.map(Some))
        .map(row)
        .collect();
    List::new(title, rows)
}
