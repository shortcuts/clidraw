use crate::model::{Canvas, Element, Id};
use std::collections::HashMap;

const K1: f64 = 1.2;
const B: f64 = 0.75;

/// One searchable unit: a canvas name or one element's text.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Doc {
    pub canvas: String,
    /// `None` for the canvas name.
    pub id: Option<Id>,
    /// `name`, `label` or `text`.
    pub field: &'static str,
    pub text: String,
}

/// Flattens canvases into one document per name and one per element with text.
pub fn documents(canvases: &[(String, Canvas)]) -> Vec<Doc> {
    let mut docs = vec![];
    for (name, canvas) in canvases {
        docs.push(Doc {
            canvas: name.clone(),
            id: None,
            field: "name",
            text: name.clone(),
        });
        for el in &canvas.elements {
            let (field, mut parts) = match el {
                Element::Shape(s) => (
                    "label",
                    std::iter::once(s.label.clone())
                        .chain(s.side_labels.iter().map(|l| l.text.clone()))
                        .collect::<Vec<_>>(),
                ),
                Element::Line(l) => ("label", vec![l.label.clone()]),
                Element::Text(t) => ("text", vec![t.text.clone()]),
            };
            parts.extend(el.meta().iter().flat_map(|(k, v)| [k.clone(), v.clone()]));
            let text = parts.join(" ");
            if !tokens(&text).is_empty() {
                docs.push(Doc {
                    canvas: name.clone(),
                    id: Some(el.id().clone()),
                    field,
                    text,
                });
            }
        }
    }
    docs
}

fn tokens(s: &str) -> Vec<String> {
    s.to_lowercase()
        .split(|c: char| !c.is_alphanumeric())
        .filter(|t| !t.is_empty())
        .map(str::to_string)
        .collect()
}

/// Okapi BM25 over `docs`. Returns `(index, score)` best first; ties keep input order; zero scores drop.
pub fn rank(docs: &[String], query: &str) -> Vec<(usize, f64)> {
    let docs: Vec<Vec<String>> = docs.iter().map(|d| tokens(d)).collect();
    let mut query = tokens(query);
    query.sort();
    query.dedup();
    let n = docs.len() as f64;
    let avg = docs.iter().map(Vec::len).sum::<usize>() as f64 / n.max(1.0);
    let mut hits: Vec<(usize, f64)> = vec![];
    for (i, doc) in docs.iter().enumerate() {
        let mut tf: HashMap<&str, f64> = HashMap::new();
        for t in doc {
            *tf.entry(t).or_default() += 1.0;
        }
        let score: f64 = query
            .iter()
            .filter_map(|q| {
                let f = *tf.get(q.as_str())?;
                let df = docs.iter().filter(|d| d.contains(q)).count() as f64;
                let idf = (1.0 + (n - df + 0.5) / (df + 0.5)).ln();
                Some(idf * f * (K1 + 1.0) / (f + K1 * (1.0 - B + B * doc.len() as f64 / avg)))
            })
            .sum();
        if score > 0.0 {
            hits.push((i, score));
        }
    }
    hits.sort_by(|a, b| b.1.total_cmp(&a.1));
    hits
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{Meta, Shape, ShapeKind, Text};

    fn strings(v: &[&str]) -> Vec<String> {
        v.iter().map(|s| s.to_string()).collect()
    }

    #[test]
    fn rank_orders_by_density_and_drops_misses() {
        let docs = strings(&[
            "payment service gateway",
            "payment payment",
            "auth",
            "Payment!",
        ]);
        let order: Vec<usize> = rank(&docs, "payment").iter().map(|h| h.0).collect();
        assert_eq!(order, vec![1, 3, 0]);
        assert!(rank(&docs, "zzz").is_empty());
        assert!(rank(&docs, "  ").is_empty());
    }

    #[test]
    fn rank_prefers_rare_terms() {
        let docs = strings(&["alpha beta", "alpha", "alpha", "beta gamma"]);
        let order: Vec<usize> = rank(&docs, "alpha gamma").iter().map(|h| h.0).collect();
        assert_eq!(order[0], 3);
    }

    #[test]
    fn documents_flatten_labels_text_and_meta() {
        let mut shape = Shape::new(ShapeKind::Rect, 0, 0, 8, 3, "Billing");
        shape.meta = Meta::from([("owner".to_string(), "ana".to_string())]);
        let text = Text {
            id: Id::from("t1"),
            x: 0,
            y: 0,
            text: "todo".into(),
            color: None,
            meta: Meta::new(),
        };
        let blank = Shape::new(ShapeKind::Rect, 0, 0, 8, 3, "");
        let canvas = Canvas {
            elements: vec![
                Element::Shape(shape),
                Element::Text(text),
                Element::Shape(blank),
            ],
            ..Canvas::default()
        };
        let docs = documents(&[("main".to_string(), canvas)]);
        let flat: Vec<(&str, &str)> = docs.iter().map(|d| (d.field, d.text.as_str())).collect();
        assert_eq!(
            flat,
            vec![
                ("name", "main"),
                ("label", "Billing owner ana"),
                ("text", "todo")
            ]
        );
        assert_eq!(docs[0].id, None);
        assert_eq!(docs[2].id, Some(Id::from("t1")));
    }
}
