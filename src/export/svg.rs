use crate::model::{corners, label_pos, wrap, Canvas, Element, LineKind, ShapeKind, Stroke};
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

fn dash(s: Stroke) -> &'static str {
    match s {
        Stroke::Solid => "",
        Stroke::Dashed => " stroke-dasharray=\"8 4\"",
        Stroke::Dotted => " stroke-dasharray=\"2 4\"",
    }
}

fn text(out: &mut String, x: i32, y: i32, anchor: &str, lines: &[&str]) {
    if lines.iter().all(|l| l.is_empty()) {
        return;
    }
    let y = y - (lines.len() as i32 - 1) * CH / 2;
    let head =
        format!("<text x=\"{x}\" y=\"{y}\" text-anchor=\"{anchor}\" dominant-baseline=\"central\"");
    if let [one] = lines {
        let _ = writeln!(out, "  {head}>{}</text>", esc(one));
        return;
    }
    let _ = writeln!(out, "  {head}>");
    for (i, l) in lines.iter().enumerate() {
        let dy = if i == 0 { 0 } else { CH };
        let _ = writeln!(out, "    <tspan x=\"{x}\" dy=\"{dy}\">{}</tspan>", esc(l));
    }
    out.push_str("  </text>\n");
}

pub fn svg(canvas: &Canvas) -> String {
    let (bx, by, bw, bh) = canvas.all_bounds().unwrap_or((0, 0, 1, 1));
    let (vx, vy) = ((bx - MARGIN) * CW, (by - MARGIN) * CH);
    let (vw, vh) = ((bw + 2 * MARGIN) * CW, (bh + 2 * MARGIN) * CH);
    let mut o = String::new();
    let _ = writeln!(o, "<svg xmlns=\"http://www.w3.org/2000/svg\" viewBox=\"{vx} {vy} {vw} {vh}\" width=\"{vw}\" height=\"{vh}\" font-family=\"monospace\" font-size=\"14\">");
    o.push_str("  <defs><marker id=\"arrow\" viewBox=\"0 0 10 10\" refX=\"9\" refY=\"5\" markerWidth=\"8\" markerHeight=\"8\" orient=\"auto-start-reverse\"><path d=\"M0 0 L10 5 L0 10 z\"/></marker></defs>\n");
    let style = |s: Stroke| format!("fill=\"none\" stroke=\"black\"{}", dash(s));
    for e in &canvas.elements {
        match e {
            Element::Shape(s) => {
                let style = style(s.stroke);
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
                let room = (s.w - 2).max(1) as usize;
                let lines: Vec<&str> = wrap(&s.label, room)
                    .into_iter()
                    .map(|r| s.label[r].trim_end_matches([' ', '\n']))
                    .collect();
                text(&mut o, x + w / 2, y + h / 2, "middle", &lines);
            }
            Element::Line(l) => {
                let cells = canvas.route(l);
                let mid = |p: (i32, i32)| (p.0 * CW + CW / 2, p.1 * CH + CH / 2);
                let marker = match l.kind {
                    LineKind::Arrow => " marker-end=\"url(#arrow)\"",
                    LineKind::ArrowStart => " marker-start=\"url(#arrow)\"",
                    LineKind::ArrowBoth => {
                        " marker-start=\"url(#arrow)\" marker-end=\"url(#arrow)\""
                    }
                    LineKind::Line => "",
                };
                let d = corners(&cells)
                    .into_iter()
                    .map(mid)
                    .enumerate()
                    .map(|(i, p)| format!("{}{} {}", if i == 0 { "M" } else { " L" }, p.0, p.1))
                    .collect::<String>();
                let _ = writeln!(
                    o,
                    "  <path d=\"{d}\" {} stroke-linejoin=\"round\"{marker}/>",
                    style(l.stroke)
                );
                let (lx, ly) = mid(label_pos(&cells, l.label_at));
                text(&mut o, lx, ly, "middle", &[&l.label]);
            }
            Element::Text(t) => text(&mut o, t.x * CW, t.y * CH + CH / 2, "start", &[&t.text]),
        }
    }
    o.push_str("</svg>\n");
    o
}
