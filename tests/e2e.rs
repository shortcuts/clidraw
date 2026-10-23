#[path = "e2e/harness.rs"]
mod harness;

use harness::Term;

#[test]
fn wheel_pans_the_canvas() {
    let tmp = tempfile::tempdir().unwrap();
    let mut t = Term::spawn(tmp.path(), &["scratch"]);
    t.draw_rect();
    let before = t.find("■");
    t.wheel_down(40, 20);
    let after = t.find("■");
    assert_eq!((after.0, after.1 + 2), before, "{}", t.screen());
}

#[test]
fn export_svg_from_the_menu() {
    let tmp = tempfile::tempdir().unwrap();
    let mut t = Term::spawn(tmp.path(), &["scratch"]);
    t.draw_rect();
    t.esc();
    t.esc();
    t.wait_for("Export SVG");
    t.key("s");
    t.wait_for("Whole canvas");
    let (col, row) = t.find("Whole canvas");
    t.click(col, row);
    t.wait_for("Export SVG to path");
    let out = tmp.path().join("o.svg");
    t.key(&"\x7f".repeat(32));
    t.key(out.to_str().unwrap());
    t.key("\r");
    t.quiet();
    assert!(std::fs::read_to_string(out).unwrap().starts_with("<svg"));
}

#[test]
fn label_caret_editing() {
    let tmp = tempfile::tempdir().unwrap();
    let mut t = Term::spawn(tmp.path(), &["scratch"]);
    t.draw_rect();
    t.double_click(16, 10);
    t.key("abd");
    t.key("\x1b[D");
    t.key("c");
    t.key("\r");
    t.wait_for("abcd");
}

#[test]
fn right_click_menu_items() {
    let tmp = tempfile::tempdir().unwrap();
    let mut t = Term::spawn(tmp.path(), &["scratch"]);
    t.draw_rect();
    let bars = |t: &Term| t.screen().matches('│').count();
    let one = bars(&t);
    t.right_click(16, 10);
    t.wait_for("Duplicate");
    for item in ["Delete", "Bring to front", "Send to back"] {
        assert!(t.screen().contains(item), "{item}\n{}", t.screen());
    }
    let (col, row) = t.find("Duplicate");
    t.click(col, row);
    assert!(
        bars(&t) > one && !t.screen().contains("Duplicate"),
        "{}",
        t.screen()
    );
}

#[test]
fn line_routing_click_and_undo() {
    let tmp = tempfile::tempdir().unwrap();
    let mut t = Term::spawn(tmp.path(), &["scratch"]);
    t.draw_rect();
    t.key("r");
    t.drag((50, 18), (62, 23));
    t.key("a");
    t.drag((22, 10), (55, 20));
    let before = t.canvas();
    let (col, row) = t.find("/ direct");
    t.click(col, row);
    let routed = t.canvas();
    assert_ne!(routed, before);
    t.key("\x1a");
    t.quiet();
    assert_eq!(t.canvas(), before);
}

#[test]
fn fit_all_glides_then_goes_idle() {
    let tmp = tempfile::tempdir().unwrap();
    let mut t = Term::spawn(tmp.path(), &["scratch"]);
    t.draw_rect();
    for _ in 0..10 {
        t.wheel_down(40, 20);
    }
    t.esc();
    t.esc();
    let panned = t.canvas();
    t.key("f");
    t.quiet();
    assert_ne!(t.canvas(), panned);
    assert_eq!(t.bytes_in(std::time::Duration::from_millis(500)), 0);
}

#[test]
fn drag_a_line_to_bend_it_and_reset_the_route() {
    let tmp = tempfile::tempdir().unwrap();
    let mut t = Term::spawn(tmp.path(), &["scratch"]);
    t.draw_rect();
    t.key("r");
    t.drag((50, 18), (62, 23));
    t.key("a");
    t.drag((22, 10), (55, 20));
    let straight = t.canvas();
    let (col, row) = t.find("╮");
    t.drag((col, row + 2), (col + 8, row + 2));
    assert_ne!(t.canvas(), straight);
    t.right_click(col + 8, row + 2);
    t.wait_for("Reset route");
    let (c, r) = t.find("Reset route");
    t.click(c, r);
    t.quiet();
    assert_eq!(t.canvas().trim_end(), straight.trim_end());
}
