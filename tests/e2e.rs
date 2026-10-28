#[path = "e2e/harness.rs"]
mod harness;

use harness::Term;

fn draw_scene(t: &mut Term) {
    t.draw_rect_at((10, 8), (22, 13));
    t.draw_rect_at((34, 5), (46, 16));
    t.draw_rect_at((60, 8), (72, 13));
}

fn cell(t: &Term, col: u16, row: u16) -> char {
    t.screen()
        .lines()
        .nth(row as usize)
        .and_then(|line| line.chars().nth(col as usize))
        .unwrap_or(' ')
}

fn assert_detours_and_points_at_b(t: &Term) {
    assert_eq!(
        cell(t, 34, 5),
        '┌',
        "obstacle O is not drawn\n{}",
        t.screen()
    );
    for row in 6..=15 {
        for col in 35..=45 {
            assert_eq!(
                cell(t, col, row),
                ' ',
                "arrow crosses obstacle O at ({col}, {row})\n{}",
                t.screen()
            );
        }
    }
    assert_eq!(t.screen().matches(['▶', '◆']).count(), 1, "{}", t.screen());
    assert!(matches!(cell(t, 59, 11), '▶' | '◆'), "{}", t.screen());
    assert_eq!(cell(t, 58, 11), '─', "{}", t.screen());
    assert_eq!(cell(t, 57, 11), '─', "{}", t.screen());
}

#[test]
fn wheel_pans_the_canvas() {
    let tmp = tempfile::tempdir().unwrap();
    let mut t = Term::spawn(tmp.path(), &["scratch"]);
    t.draw_rect();
    let before = t.find("⤡");
    t.wheel_down(40, 20);
    let after = t.find("⤡");
    assert_eq!((after.0, after.1 + 2), before, "{}", t.screen());
}

#[test]
fn export_svg_from_the_palette() {
    let tmp = tempfile::tempdir().unwrap();
    let mut t = Term::spawn(tmp.path(), &["scratch"]);
    t.draw_rect();
    t.key("\x10");
    t.key("export svg");
    t.key("\r");
    t.wait_for("Whole canvas");
    let (col, row) = t.find("Whole canvas");
    t.click(col, row);
    t.wait_for("Export SVG to path");
    let out = tmp.path().join("o.svg");
    t.key(&"\x7f".repeat(32));
    t.key(out.to_str().unwrap());
    t.key("\r");
    t.wait_for("Exported");
    assert!(std::fs::read_to_string(out).unwrap().starts_with("<svg"));
}

#[cfg(unix)]
fn signal_saves_pending_edits(sig: i32) {
    let tmp = tempfile::tempdir().unwrap();
    std::fs::create_dir_all(tmp.path().join("c")).unwrap();
    std::fs::write(tmp.path().join("c/config.toml"), "autosave_ms = 60000\n").unwrap();
    let mut t = Term::spawn(tmp.path(), &["scratch"]);
    t.draw_rect();
    t.signal_and_wait(sig);
    let out = std::process::Command::new(env!("CARGO_BIN_EXE_clidraw"))
        .args(["show", "scratch"])
        .env("CLIDRAW_HOME", tmp.path().join("c"))
        .output()
        .unwrap();
    assert!(String::from_utf8_lossy(&out.stdout).contains("\"Rect"));
}

#[cfg(unix)]
#[test]
fn sigterm_saves_pending_edits() {
    signal_saves_pending_edits(libc::SIGTERM);
}

#[cfg(unix)]
#[test]
fn sighup_saves_pending_edits() {
    signal_saves_pending_edits(libc::SIGHUP);
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
fn side_label_sits_on_the_border_drags_along_it_and_exports() {
    let tmp = tempfile::tempdir().unwrap();
    let mut t = Term::spawn(tmp.path(), &["scratch"]);
    t.draw_rect();
    t.double_click(12, 8);
    t.key("yes");
    t.key("\r");
    t.wait_for("┌yes");
    let (col, row) = t.find("yes");
    t.drag((col, row), (col + 6, row));
    t.wait_for("┌─────yes");
    t.key("\x10");
    t.key("export svg");
    t.key("\r");
    t.wait_for("Whole canvas");
    let (col, row) = t.find("Whole canvas");
    t.click(col, row);
    t.wait_for("Export SVG to path");
    let out = tmp.path().join("o.svg");
    t.key(&"\x7f".repeat(32));
    t.key(out.to_str().unwrap());
    t.key("\r");
    t.wait_for("Exported");
    assert!(std::fs::read_to_string(out)
        .unwrap()
        .contains(">yes</text>"));
}

#[test]
fn right_click_menu_items() {
    let tmp = tempfile::tempdir().unwrap();
    let mut t = Term::spawn(tmp.path(), &["scratch"]);
    t.draw_rect();
    let bars = |t: &Term| t.screen().matches('│').count();
    let one = bars(&t);
    t.right_click(10, 10);
    t.wait_for("Duplicate");
    for item in ["Delete", "Bring to front", "Raise", "Lower", "Send to back"] {
        assert!(t.screen().contains(item), "{item}\n{}", t.screen());
    }
    let (col, row) = t.find("Duplicate");
    t.click(col, row);
    t.wait_gone("Duplicate");
    assert!(bars(&t) > one, "{}", t.screen());
}

#[test]
fn fit_all_glides_then_goes_idle() {
    let tmp = tempfile::tempdir().unwrap();
    let mut t = Term::spawn(tmp.path(), &["scratch"]);
    t.draw_rect();
    for _ in 0..10 {
        t.wheel_down(40, 20);
    }
    let panned = t.canvas();
    t.key("z");
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
    t.wait_gone("Reset route");
    assert_eq!(t.canvas().trim_end(), straight.trim_end());
}

#[test]
fn arrow_drag_between_shapes_detours_mid_drag_on_release_and_after_a_click_elsewhere() {
    let tmp = tempfile::tempdir().unwrap();
    let mut t = Term::spawn(tmp.path(), &["scratch"]);
    draw_scene(&mut t);
    t.key("a");
    t.press(22, 10);
    t.move_to(60, 10);
    assert_detours_and_points_at_b(&t);
    let held = t.canvas().replace('◆', "▶");
    t.release(60, 10);
    assert_detours_and_points_at_b(&t);
    assert_eq!(t.canvas(), held);
    t.click(90, 28);
    assert_detours_and_points_at_b(&t);
}

#[test]
fn dragging_a_placed_head_to_another_shape_moves_the_arrow() {
    let tmp = tempfile::tempdir().unwrap();
    let mut t = Term::spawn(tmp.path(), &["scratch"]);
    draw_scene(&mut t);
    t.draw_rect_at((60, 20), (72, 25));
    t.key("a");
    t.drag((22, 10), (59, 10));
    t.click(28, 10);
    t.press(59, 10);
    t.move_to(59, 22);
    t.release(59, 22);
    assert_eq!(t.screen().matches('▶').count(), 1, "{}", t.screen());
    assert_eq!(cell(&t, 59, 22), '▶', "{}", t.screen());
    assert!(
        (0..59).all(|col| cell(&t, col, 10) != '▶'),
        "{}",
        t.screen()
    );
}
