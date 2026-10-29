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
}
