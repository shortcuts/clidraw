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
    pub kind: String,
    pub label: String,
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
            kind: "canvas".into(),
            label: name.clone(),
            field: "name",
            text: name.clone(),
        });
        for el in &canvas.elements {
            let (kind, label, field, mut parts) = match el {
                Element::Shape(s) => (
                    format!("{:?}", s.kind).to_lowercase(),
                    s.label.clone(),
                    "label",
                    std::iter::once(s.label.clone())
                        .chain(s.side_labels.iter().map(|l| l.text.clone()))
                        .collect::<Vec<_>>(),
                ),
                Element::Line(l) => (
                    "line".into(),
                    l.label.clone(),
                    "label",
                    vec![l.label.clone()],
                ),
                Element::Text(t) => ("text".into(), t.text.clone(), "text", vec![t.text.clone()]),
            };
            parts.extend(el.meta().iter().flat_map(|(k, v)| [k.clone(), v.clone()]));
            let text = parts.join(" ");
            if !tokens(&text).is_empty() {
                docs.push(Doc {
                    canvas: name.clone(),
                    id: Some(el.id().clone()),
                    kind,
                    label,
                    field,
                    text,
                });
            }
        }
    }
    docs
}

pub const MAX_HITS: usize = 50;

pub fn hits(canvases: &[(String, Canvas)], query: &str) -> Vec<Doc> {
    let docs = documents(canvases);
    let texts: Vec<String> = docs.iter().map(|d| d.text.clone()).collect();
    find(query, &texts)
        .into_iter()
        .take(MAX_HITS)
        .map(|i| docs[i].clone())
        .collect()
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

/// Words that mean the same to a user; `find` matches a query word through its group.
pub const SYNONYMS: &[&[&str]] = &[
    &["create", "new", "add"],
    &["switch", "list", "open"],
    &["remove", "erase", "delete"],
    &["exit", "close", "quit"],
    &["front", "raise"],
    &["back", "lower"],
    &["redo", "undo"],
    &["box", "rect"],
    &["text", "label"],
];

fn group(word: &str) -> &'static [&'static str] {
    SYNONYMS
        .iter()
        .find(|g| g.contains(&word))
        .copied()
        .unwrap_or(&[])
}

fn subsequence(query: &str, label: &str) -> bool {
    let mut label = label.chars();
    query
        .chars()
        .filter(|c| !c.is_whitespace())
        .all(|q| label.any(|c| c == q))
}

fn word_start(label: &str, query: &str) -> bool {
    label.match_indices(query).any(|(i, _)| {
        label[..i]
            .chars()
            .next_back()
            .is_none_or(|c| !c.is_alphanumeric())
    })
}

/// Indices into `labels` that match `query`, best first: prefix or word-start, substring,
/// every word, every word or synonym, then subsequence. BM25 orders each tier; ties keep input order.
pub fn find(query: &str, labels: &[String]) -> Vec<usize> {
    let query = query.trim().to_lowercase();
    if query.is_empty() {
        return vec![];
    }
    let words = tokens(&query);
    let expanded = words
        .iter()
        .flat_map(|w| std::iter::once(w.as_str()).chain(group(w).iter().copied()))
        .collect::<Vec<_>>()
        .join(" ");
    let score = |hits: Vec<(usize, f64)>| -> HashMap<usize, f64> { hits.into_iter().collect() };
    let (plain, wide) = (score(rank(labels, &query)), score(rank(labels, &expanded)));
    let mut hits: Vec<(usize, f64, usize)> = vec![];
    for (i, label) in labels.iter().enumerate() {
        let label = label.to_lowercase();
        let have = tokens(&label);
        let tier = if label.starts_with(&query) || word_start(&label, &query) {
            0
        } else if label.contains(&query) {
            1
        } else if !words.is_empty() && words.iter().all(|w| have.contains(w)) {
            2
        } else if !words.is_empty()
            && words
                .iter()
                .all(|w| have.contains(w) || group(w).iter().any(|s| have.iter().any(|h| h == s)))
        {
            3
        } else if subsequence(&query, &label) {
            4
        } else {
            continue;
        };
        let by = if tier == 3 { &wide } else { &plain };
        hits.push((tier, by.get(&i).copied().unwrap_or(0.0), i));
    }
    hits.sort_by(|a, b| a.0.cmp(&b.0).then(b.1.total_cmp(&a.1)).then(a.2.cmp(&b.2)));
    hits.into_iter().map(|h| h.2).collect()
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
    fn find_matches_synonyms_and_ranks_prefix_first() {
        let labels = strings(&["Create canvas", "Switch canvas", "New note", "Newton"]);
        assert_eq!(find("new", &labels), vec![2, 3, 0]);
        assert_eq!(find("list", &labels), vec![1]);
        assert_eq!(find("switch canvas", &labels), vec![1]);
        let labels = strings(&["Open file", "Switch canvas"]);
        assert_eq!(find("switch", &labels), vec![1, 0]);
    }

    #[test]
    fn find_keeps_subsequence_and_ignores_blank_query() {
        let labels = strings(&["File: Export SVG", "Quit"]);
        assert_eq!(find("exsvg", &labels), vec![0]);
        assert!(find("  ", &labels).is_empty());
        assert!(find("zzz", &labels).is_empty());
        assert!(find(":", &strings(&["Quit"])).is_empty());
    }

    #[test]
    fn synonym_table_is_lowercase_disjoint_groups() {
        let mut seen = std::collections::HashSet::new();
        for g in SYNONYMS {
            assert!(g.len() >= 2);
            for w in *g {
                assert_eq!(*w, w.to_lowercase());
                assert!(seen.insert(*w), "{w} in two groups");
            }
        }
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

    fn canvas_with(elements: Vec<Element>) -> Canvas {
        Canvas {
            elements,
            ..Canvas::default()
        }
    }

    #[test]
    fn hits_reports_the_canvas_name() {
        let canvases = [("billing-plans".to_string(), Canvas::default())];
        let hits = hits(&canvases, "plans");
        assert_eq!(
            hits,
            vec![Doc {
                canvas: "billing-plans".into(),
                id: None,
                kind: "canvas".into(),
                label: "billing-plans".into(),
                field: "name",
                text: "billing-plans".into(),
            }]
        );
    }

    #[test]
    fn hits_reports_frame_kind_and_metadata_match() {
        let mut frame = Shape::new(ShapeKind::Frame, 0, 0, 20, 6, "Billing review");
        frame.id = Id::from("f");
        let mut shape = Shape::new(ShapeKind::Rect, 0, 0, 8, 3, "Login");
        shape.id = Id::from("m");
        shape.meta = Meta::from([("team".to_string(), "billing".to_string())]);
        let canvases = [(
            "ops".to_string(),
            canvas_with(vec![Element::Shape(frame), Element::Shape(shape)]),
        )];
        let frames = hits(&canvases, "review");
        assert_eq!(frames.len(), 1);
        assert_eq!(frames[0].kind, "frame");
        assert_eq!(frames[0].label, "Billing review");
        assert_eq!(frames[0].field, "label");
        let meta = hits(&canvases, "team");
        assert_eq!(meta.len(), 1);
        assert_eq!(meta[0].id, Some(Id::from("m")));
        assert_eq!(meta[0].kind, "rect");
        assert_eq!(meta[0].label, "Login");
        assert_eq!(meta[0].field, "label");
    }

    #[test]
    fn hits_ranks_exact_before_synonym() {
        let mut create = Shape::new(ShapeKind::Rect, 0, 0, 8, 3, "Create canvas");
        create.id = Id::from("a");
        let mut new = Shape::new(ShapeKind::Rect, 0, 0, 8, 3, "New note");
        new.id = Id::from("b");
        let canvases = [(
            "main".to_string(),
            canvas_with(vec![Element::Shape(create), Element::Shape(new)]),
        )];
        let ids: Vec<Option<Id>> = hits(&canvases, "new").into_iter().map(|d| d.id).collect();
        assert_eq!(ids, vec![Some(Id::from("b")), Some(Id::from("a"))]);
    }

    #[test]
    fn hits_is_empty_for_a_blank_query() {
        let canvases = [("main".to_string(), Canvas::default())];
        assert!(hits(&canvases, "  ").is_empty());
    }

    #[test]
    fn hits_stops_at_max_hits() {
        let elements = (0..MAX_HITS + 10)
            .map(|i| Element::Shape(Shape::new(ShapeKind::Rect, i as i32, 0, 8, 3, "Billing")))
            .collect();
        let canvases = [("main".to_string(), canvas_with(elements))];
        assert_eq!(hits(&canvases, "billing").len(), MAX_HITS);
    }
}
