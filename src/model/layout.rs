use super::{Canvas, Command, Element, End, Id, ShapeKind};
use serde::Deserialize;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Deserialize)]
pub enum Dir {
    #[serde(rename = "td")]
    TopDown,
    #[serde(rename = "lr")]
    LeftRight,
}

impl Canvas {
    /// Moves the selected shapes into layers by the lines bound between them. Empty when fewer
    /// than two shapes are selected or nothing moves.
    pub fn layout(&self, selection: &[Id], dir: Dir) -> Vec<Command> {
        let lr = dir == Dir::LeftRight;
        // (id, x, y, main size, cross size); `main` runs along the layers.
        let mut nodes: Vec<(Id, i32, i32, i32, i32)> = self
            .elements
            .iter()
            .filter_map(|e| match e {
                Element::Shape(s) if selection.contains(&s.id) && s.kind != ShapeKind::Frame => {
                    let (m, c) = if lr { (s.w, s.h) } else { (s.h, s.w) };
                    Some((s.id.clone(), s.x, s.y, m, c))
                }
                _ => None,
            })
            .collect();
        if nodes.len() < 2 {
            return vec![];
        }
        nodes.sort_by_key(|n| if lr { (n.2, n.1) } else { (n.1, n.2) });
        let n = nodes.len();
        let find = |id: &Id| nodes.iter().position(|x| &x.0 == id);
        let mut adj = vec![vec![]; n];
        for e in &self.elements {
            if let Element::Line(l) = e {
                if let (End::Bound(a, _), End::Bound(b, _)) = (&l.from, &l.to) {
                    if let (Some(a), Some(b)) = (find(a), find(b)) {
                        if a != b {
                            adj[a].push(b);
                        }
                    }
                }
            }
        }
        for a in &mut adj {
            a.sort_unstable();
            a.dedup();
        }

        // Depth-first search drops back edges, and reversed post-order is a topological order.
        let (mut state, mut post) = (vec![0u8; n], vec![]);
        let mut dag = vec![vec![]; n];
        for root in 0..n {
            visit(root, &adj, &mut state, &mut post, &mut dag);
        }
        let mut layer = vec![0usize; n];
        for &u in post.iter().rev() {
            for &v in &dag[u] {
                layer[v] = layer[v].max(layer[u] + 1);
            }
        }
        let mut preds = vec![vec![]; n];
        for (u, vs) in dag.iter().enumerate() {
            for &v in vs {
                preds[v].push(u);
            }
        }

        let mut layers: Vec<Vec<usize>> = vec![vec![]; layer.iter().max().map_or(0, |m| m + 1)];
        for (i, &l) in layer.iter().enumerate() {
            layers[l].push(i);
        }
        // shortcut: one barycenter sweep, upgrade if users report crossings.
        let mut pos = vec![0i32; n];
        for (k, members) in layers.iter_mut().enumerate() {
            if k > 0 {
                let key = |i: usize| {
                    let p: Vec<i32> = preds[i]
                        .iter()
                        .filter(|&&p| layer[p] == k - 1)
                        .map(|&p| pos[p])
                        .collect();
                    match p.len() {
                        0 => pos[i] * 1000,
                        c => p.iter().sum::<i32>() * 1000 / c as i32,
                    }
                };
                members.sort_by_key(|&i| key(i));
            }
            for (at, &i) in members.iter().enumerate() {
                pos[i] = at as i32;
            }
        }

        let (layer_gap, side_gap) = if lr { (6, 1) } else { (3, 4) };
        let span = |m: &[usize]| {
            m.iter().map(|&i| nodes[i].4).sum::<i32>() + side_gap * (m.len() as i32 - 1)
        };
        let widest = layers.iter().map(|m| span(m)).max().unwrap_or(0);
        let ox = nodes.iter().map(|n| n.1).min().unwrap_or(0);
        let oy = nodes.iter().map(|n| n.2).min().unwrap_or(0);
        let mut cmds = vec![];
        let mut main = 0;
        for members in &layers {
            let mut cross = (widest - span(members)) / 2;
            for &i in members {
                let (id, x, y, _, c) = &nodes[i];
                let (nx, ny) = if lr {
                    (ox + main, oy + cross)
                } else {
                    (ox + cross, oy + main)
                };
                if (nx, ny) != (*x, *y) {
                    cmds.push(Command::Move {
                        id: id.clone(),
                        dx: nx - x,
                        dy: ny - y,
                    });
                }
                cross += c + side_gap;
            }
            main += members.iter().map(|&i| nodes[i].3).max().unwrap_or(0) + layer_gap;
        }
        cmds
    }
}

fn visit(
    u: usize,
    adj: &[Vec<usize>],
    state: &mut [u8],
    post: &mut Vec<usize>,
    dag: &mut [Vec<usize>],
) {
    if state[u] != 0 {
        return;
    }
    state[u] = 1;
    for &v in &adj[u] {
        if state[v] == 1 {
            continue;
        }
        dag[u].push(v);
        visit(v, adj, state, post, dag);
    }
    state[u] = 2;
    post.push(u);
}
