use crate::model::{label_pos, Canvas, Element, LineKind, ShapeKind};
use std::fmt::Write;

const CW: i32 = 8;
const CH: i32 = 16;
const MARGIN: i32 = 2;

fn esc(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

fn text(out: &mut String, x: i32, y: i32, anchor: &str, s: &str) {
    if !s.is_empty() {
        let _ = writeln!(out, "  <text x=\"{x}\" y=\"{y}\" text-anchor=\"{anchor}\" dominant-baseline=\"central\">{}</text>", esc(s));
    }
}

pub fn svg(canvas: &Canvas) -> String {
    let (bx, by, bw, bh) = canvas.all_bounds().unwrap_or((0, 0, 1, 1));
    let (vx, vy) = ((bx - MARGIN) * CW, (by - MARGIN) * CH);
    let (vw, vh) = ((bw + 2 * MARGIN) * CW, (bh + 2 * MARGIN) * CH);
    let mut o = String::new();
    let _ = writeln!(o, "<svg xmlns=\"http://www.w3.org/2000/svg\" viewBox=\"{vx} {vy} {vw} {vh}\" width=\"{vw}\" height=\"{vh}\" font-family=\"monospace\" font-size=\"14\">");
    o.push_str("  <defs><marker id=\"arrow\" viewBox=\"0 0 10 10\" refX=\"9\" refY=\"5\" markerWidth=\"8\" markerHeight=\"8\" orient=\"auto\"><path d=\"M0 0 L10 5 L0 10 z\"/></marker></defs>\n");
    let style = "fill=\"none\" stroke=\"black\"";
    for e in &canvas.elements {
        match e {
            Element::Shape(s) => {
                let (x, y, w, h) = (s.x * CW, s.y * CH, s.w * CW, s.h * CH);
                match s.kind {
                    ShapeKind::Rect => {
                        let _ = writeln!(
                            o,
                            "  <rect x=\"{x}\" y=\"{y}\" width=\"{w}\" height=\"{h}\" {style}/>"
                        );
                    }
                    ShapeKind::Ellipse => {
                        let _ = writeln!(
                            o,
                            "  <ellipse cx=\"{}\" cy=\"{}\" rx=\"{}\" ry=\"{}\" {style}/>",
                            x + w / 2,
                            y + h / 2,
                            w / 2,
                            h / 2
                        );
                    }
                    ShapeKind::Diamond => {
                        let _ = writeln!(
                            o,
                            "  <polygon points=\"{},{} {},{} {},{} {},{}\" {style}/>",
                            x + w / 2,
                            y,
                            x + w,
                            y + h / 2,
                            x + w / 2,
                            y + h,
                            x,
                            y + h / 2
                        );
                    }
                }
                text(&mut o, x + w / 2, y + h / 2, "middle", &s.label);
            }
            Element::Line(l) => {
                let (a, b) = canvas.line_ends(l);
                let mid = |p: (i32, i32)| (p.0 * CW + CW / 2, p.1 * CH + CH / 2);
                let (pa, pb) = (mid(a), mid(b));
                let marker = if l.kind == LineKind::Arrow {
                    " marker-end=\"url(#arrow)\""
                } else {
                    ""
                };
                let _ = writeln!(
                    o,
                    "  <path d=\"M{} {} L{} {} L{} {}\" {style}{marker}/>",
                    pa.0, pa.1, pb.0, pa.1, pb.0, pb.1
                );
                let (lx, ly) = mid(label_pos(a, b));
                text(&mut o, lx, ly, "middle", &l.label);
            }
            Element::Text(t) => text(&mut o, t.x * CW, t.y * CH + CH / 2, "start", &t.text),
        }
    }
    o.push_str("</svg>\n");
    o
}
