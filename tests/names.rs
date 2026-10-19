use clidraw::names::generate;

#[test]
fn skips_taken_names() {
    let name = generate(|n| !n.starts_with('s'));
    assert!(name.starts_with('s') && name.contains('-'), "{name}");
}
