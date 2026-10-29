use crate::model::{
    corners, label_pos, side_label_at, wrap, Border, Canvas, Element, LineKind, Meta, Paint,
    ShapeKind, Stroke, TextAlign, FILL_ALPHA,
};
use std::fmt::Write;

const CW: i32 = 8;
const CH: i32 = 16;
const MARGIN: i32 = 2;
const FRAME_GREY: &str = "#767676";

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

fn hex(p: Paint) -> String {
    let (r, g, b) = p.rgb();
    format!("#{r:02x}{g:02x}{b:02x}")
}

fn text(
    out: &mut String,
    (x, y): (i32, i32),
    anchor: &str,
    lines: &[&str],
    (ink, bold): (Option<String>, bool),
) {
    if lines.iter().all(|l| l.is_empty()) {
        return;
    }
    let y = y - (lines.len() as i32 - 1) * CH / 2;
    let head =
        format!("<text x=\"{x}\" y=\"{y}\" text-anchor=\"{anchor}\" dominant-baseline=\"central\"");
    let head = if bold {
        format!("{head} font-weight=\"bold\"")
    } else {
        head
    };
    let head = match ink {
        Some(c) => format!("{head} fill=\"{c}\""),
        None => head,
    };
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

fn linked(o: &mut String, start: usize, link: &Option<String>) {
    if let Some(url) = link {
        o.insert_str(start, &format!("  <a href=\"{}\">\n", esc(url)));
        o.push_str("  </a>\n");
    }
}

/// Wraps the element written since `start` in a group whose `<desc>` lists the metadata.
fn annotate(o: &mut String, start: usize, meta: &Meta) {
    if meta.is_empty() {
        return;
    }
    let desc = meta
        .iter()
        .map(|(k, v)| format!("{}: {}", esc(k), esc(v)))
        .collect::<Vec<_>>()
        .join("\n");
    o.insert_str(start, &format!("  <g>\n    <desc>{desc}</desc>\n"));
    o.push_str("  </g>\n");
}

pub fn svg(canvas: &Canvas) -> String {
    svg_padded(canvas, MARGIN)
}

/// `pad` is the blank margin in cells around the drawing.
pub fn svg_padded(canvas: &Canvas, pad: i32) -> String {
    let find = canvas.shape_lookup();
    let router = canvas.router();
    let (bx, by, bw, bh) = canvas
        .all_bounds_with(&find, &router)
        .unwrap_or((0, 0, 1, 1));
    let (vx, vy) = ((bx - pad) * CW, (by - pad) * CH);
    let (vw, vh) = ((bw + 2 * pad) * CW, (bh + 2 * pad) * CH);
    let mut o = String::new();
    let _ = writeln!(o, "<svg xmlns=\"http://www.w3.org/2000/svg\" viewBox=\"{vx} {vy} {vw} {vh}\" width=\"{vw}\" height=\"{vh}\" font-family=\"monospace\" font-size=\"14\">");
    o.push_str("  <defs><marker id=\"arrow\" viewBox=\"0 0 10 10\" refX=\"9\" refY=\"5\" markerWidth=\"8\" markerHeight=\"8\" orient=\"auto-start-reverse\"><path d=\"M0 0 L10 5 L0 10 z\" fill=\"context-stroke\"/></marker></defs>\n");
    let style = |s: Stroke, ink: Option<Paint>, fill: Option<Paint>, unset: &str| {
        let fill = fill.map_or("none".to_string(), hex);
        let opacity = if fill == "none" {
            String::new()
        } else {
            format!(" fill-opacity=\"{}\"", f64::from(FILL_ALPHA) / 100.0)
        };
        let stroke = ink.map_or(unset.to_string(), hex);
        format!("fill=\"{fill}\"{opacity} stroke=\"{stroke}\"{}", dash(s))
    };
    for e in canvas.stacking().map(|i| &canvas.elements[i]) {
        let start = o.len();
        match e {
            Element::Shape(s) => {
                let width = match s.border {
                    Border::Double => " stroke-width=\"2\"",
                    Border::Heavy => " stroke-width=\"3\"",
                    _ => "",
                };
                let frame = s.kind == ShapeKind::Frame;
                let (stroke, unset) = match (frame, s.stroke) {
                    (true, Stroke::Solid) => (Stroke::Dotted, FRAME_GREY),
                    (true, st) => (st, FRAME_GREY),
                    (false, st) => (st, "black"),
                };
                let arc = format!("{}{width}", style(stroke, s.color, None, unset));
                let style = format!("{}{width}", style(stroke, s.color, s.fill, unset));
                let rx = if s.border == Border::Rounded || s.kind == ShapeKind::Stadium {
                    " rx=\"8\""
                } else {
                    ""
                };
                let (x, y, w, h) = (s.x * CW, s.y * CH, s.w * CW, s.h * CH);
                match s.kind {
                    ShapeKind::Rect | ShapeKind::Stadium | ShapeKind::Frame => {
                        let _ = writeln!(
                            o,
                            "  <rect x=\"{x}\" y=\"{y}\" width=\"{w}\" height=\"{h}\"{rx} {style}/>"
                        );
                    }
                    ShapeKind::Cylinder => {
                        let (rx, ry) = (w / 2, CH);
                        let _ = writeln!(
                            o,
                            "  <path d=\"M {x},{} A {rx} {ry} 0 0 1 {},{} V {} A {rx} {ry} 0 0 1 {x},{} Z\" {style}/>",
                            y + ry,
                            x + w,
                            y + ry,
                            y + h - ry,
                            y + h - ry
                        );
                        let _ = writeln!(
                            o,
                            "  <path d=\"M {x},{} A {rx} {ry} 0 0 0 {},{}\" {arc}/>",
                            y + ry,
                            x + w,
                            y + ry
                        );
                    }
                    ShapeKind::Parallelogram => {
                        let k = s.inset() * CW;
                        let _ = writeln!(
                            o,
                            "  <polygon points=\"{},{} {},{} {},{} {},{}\" {style}/>",
                            x + k,
                            y,
                            x + w,
                            y,
                            x + w - k,
                            y + h,
                            x,
                            y + h
                        );
                    }
                    ShapeKind::Hexagon => {
                        let k = s.inset() * CW;
                        let _ = writeln!(
                            o,
                            "  <polygon points=\"{},{} {},{} {},{} {},{} {},{} {},{}\" {style}/>",
                            x + k,
                            y,
                            x + w - k,
                            y,
                            x + w,
                            y + h / 2,
                            x + w - k,
                            y + h,
                            x + k,
                            y + h,
                            x,
                            y + h / 2
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
                if s.kind == ShapeKind::Frame {
                    let label = s.label.replace('\n', " ");
                    let ink = s.color.map(hex).unwrap_or(FRAME_GREY.into());
                    text(&mut o, (x + CW, y), "start", &[&label], (Some(ink), s.bold));
                    annotate(&mut o, start, &s.meta);
                    linked(&mut o, start, &s.link);
                    continue;
                }
                let room = (s.w - 2).max(1) as usize;
                let lines: Vec<&str> = wrap(&s.label, room)
                    .into_iter()
                    .map(|r| s.label[r].trim_end_matches([' ', '\n']))
                    .collect();
                let (tx, anchor) = match s.align {
                    TextAlign::Left => (x + CW, "start"),
                    TextAlign::Center => (x + w / 2, "middle"),
                    TextAlign::Right => (x + w - CW, "end"),
                };
                text(
                    &mut o,
                    (tx, y + h / 2),
                    anchor,
                    &lines,
                    (s.color.map(hex), s.bold),
                );
                for l in &s.side_labels {
                    let (lx, ly, shown) = side_label_at(s, l);
                    let at = (lx * CW, ly * CH + CH / 2);
                    text(&mut o, at, "start", &[shown], (s.color.map(hex), false));
                }
                annotate(&mut o, start, &s.meta);
                linked(&mut o, start, &s.link);
            }
            Element::Line(l) => {
                let cells = canvas.route_with(l, &find, &router);
                let mid = |p: (i32, i32)| (p.0 * CW + CW / 2, p.1 * CH + CH / 2);
                let marker = match l.kind {
                    LineKind::Arrow => " marker-end=\"url(#arrow)\"",
                    LineKind::ArrowStart => " marker-start=\"url(#arrow)\"",
                    LineKind::ArrowBoth => {
                        " marker-start=\"url(#arrow)\" marker-end=\"url(#arrow)\""
                    }
                    LineKind::Line => "",
                };
                // Mermaid draws one dotted edge style, so dashed edges share it.
                let stroke = match l.stroke {
                    Stroke::Dashed => Stroke::Dotted,
                    s => s,
                };
                let pts = corners(&cells);
                let d = pts
                    .into_iter()
                    .map(mid)
                    .enumerate()
                    .map(|(i, p)| format!("{}{} {}", if i == 0 { "M" } else { " L" }, p.0, p.1))
                    .collect::<String>();
                let _ = writeln!(
                    o,
                    "  <path d=\"{d}\" {} stroke-linejoin=\"round\"{marker}/>",
                    style(stroke, l.color, None, "black")
                );
                let (lx, ly) = mid(label_pos(&cells, l.label_at));
                text(
                    &mut o,
                    (lx, ly),
                    "middle",
                    &[&l.label],
                    (l.color.map(hex), false),
                );
                annotate(&mut o, start, &l.meta);
            }
            Element::Text(t) => {
                text(
                    &mut o,
                    (t.x * CW, t.y * CH + CH / 2),
                    "start",
                    &[&t.text],
                    (t.color.map(hex), false),
                );
                annotate(&mut o, start, &t.meta);
            }
        }
    }
    o.push_str("</svg>\n");
    o
}
