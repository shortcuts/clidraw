//! Scripted live check: runs the real binary in a pty and prints the screen.
//! `DRIVE=steps.txt cargo test --test drive -- --ignored --nocapture`
//! Steps, one per line: `key <text>` (\r \x10 \x1b[D escapes work), `click|rclick|dclick <col> <row>`,
//! `drag <c1> <r1> <c2> <r2>`, `wheel <col> <row>`, `wait <text>`, `click_text <text>`, `screen`.
// The harness serves e2e too, so drive leaves some of its methods unused.
#[allow(dead_code)]
#[path = "e2e/harness.rs"]
mod harness;

fn unescape(s: &str) -> String {
    s.replace("\\r", "\r")
        .replace("\\x10", "\x10")
        .replace("\\x7f", "\x7f")
        .replace("\\x1b", "\x1b")
}

#[test]
#[ignore = "run on demand with DRIVE=<steps file>"]
fn drive() {
    let steps =
        std::fs::read_to_string(std::env::var("DRIVE").expect("set DRIVE=<steps file>")).unwrap();
    let tmp = tempfile::tempdir().unwrap();
    let mut t = harness::Term::spawn(tmp.path(), &["scratch"]);
    for (n, line) in steps
        .lines()
        .map(str::trim)
        .enumerate()
        .filter(|(_, l)| !l.is_empty())
    {
        let (cmd, rest) = line.split_once(' ').unwrap_or((line, ""));
        let num = |i: usize| -> u16 {
            rest.split(' ')
                .nth(i)
                .and_then(|v| v.parse().ok())
                .unwrap_or_else(|| panic!("line {}: bad number in {line:?}", n + 1))
        };
        match cmd {
            "key" => t.key(&unescape(rest)),
            "click" => t.click(num(0), num(1)),
            "rclick" => t.right_click(num(0), num(1)),
            "dclick" => t.double_click(num(0), num(1)),
            "drag" => t.drag((num(0), num(1)), (num(2), num(3))),
            "wheel" => t.wheel_down(num(0), num(1)),
            "wait" => t.wait_for(rest),
            "click_text" => {
                let (c, r) = t.find(rest);
                t.click(c + 1, r);
            }
            "screen" => println!("--- line {} ---\n{}", n + 1, t.screen()),
            _ => panic!("line {}: unknown step {cmd:?}", n + 1),
        }
    }
    println!("--- final ---\n{}", t.screen());
}
