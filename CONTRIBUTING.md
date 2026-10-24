# Contributing

This file holds the rules for contributors and agents. The code is the source of truth. Each rule that the code shows has a Good and a Bad example from this repo.

## Constraints

The tool must be minimal, fast, effective and work on any OS.

`Makefile:34-36` uses only POSIX shell and `$HOME`, so the clean target runs on macOS and Linux.

```make
rm -rf "$${CLIDRAW_HOME:-$${XDG_CONFIG_HOME:-$$HOME/.config}/clidraw}" "$${XDG_STATE_HOME:-$$HOME/.local/state}/clidraw"
```

- Good: the target above.
- Bad: n/a: no violation in the code.

A line with bends routes plain, with no obstacle search, so an edit to an unrelated shape never reroutes it. Only the pin moments call the router: drawing or moving a line, moving or resizing a shape at a line end, and Arrange. Each pin runs in the same undo step as its edit.

```rust
for cmd in s.canvas.pin_cmds(&rerouted(s, &action)) {
    run(s, &mut inv, cmd);
}
```

- Good: `Canvas::pin_cmds` stores the automatic route as `bends` after the edit.
- Bad: calling `Router::around` for a bent leg in `route_with`, so a new shape reroutes a line that nobody edited.

A line avoids the actual route of every earlier line, so two lines never share a segment.

```rust
self.earlier(canvas, find, l, &mut deps)
```

- Good: `Router::around` reads an earlier line through `earlier`, so a second line that detours a shape takes another lane.
- Bad: reading an earlier line with `plain_with`, so two lines that both detour a shape share the detour row.

## Workflow

Run these commands from the repo root.

- `make check` runs `fmt-check`, `lint`, `test` and `perf`. CI runs it on Ubuntu and macOS.
- `make test` runs `cargo test`.
- `make lint` runs clippy with `-D warnings`.
- `make fmt` formats the code. `make fmt-check` only reports the differences.
- `make perf` checks the budgets. See [Performance](#performance).
- `make profile` records the drag scenario with samply. See [Profile](#profile).
- `make release` builds the release binary.
- `cargo test --test e2e` runs the real binary in a pty and asserts on the parsed screen.

## Tests

- Export changes need a golden-file update in `tests/golden/`. Review the diff by eye.
- A bug fix starts with a failing test.
- A bug fix test fails before the fix: run it and see the failure first.
- A change to rendering, hit testing, input handling or save passes `make perf`.
- A perf fix starts with a perf test that fails against the budget.
- The budgets are in the [Performance table](README.md#performance) of the README.

`tests/golden.rs:142-149` compares the whole export with its golden file. `UPDATE_GOLDEN` rewrites the file.

```rust
fn check(name: &str, actual: &str) {
    let path = format!("{}/tests/golden/{name}", env!("CARGO_MANIFEST_DIR"));
    if std::env::var_os("UPDATE_GOLDEN").is_some() {
        std::fs::write(&path, actual).unwrap();
    }
    let expected = std::fs::read_to_string(&path).unwrap();
    assert_eq!(actual, expected, "golden file {name} differs");
}
```

- Good: the `check` helper above.
- Bad: `tests/golden.rs:165` asserts only that the output contains `<svg`. That line is fine for the empty canvas. A full export needs a golden file.

## Diagram kinds

A diagram kind reads and writes one text format. Each kind implements the `DiagramKind` trait in `src/diagram/mod.rs`. Flowchart is the only kind now.

- Add a kind in its own module under `src/diagram/`. Implement `keywords`, `import`, `export` and `validate`.
- Register the kind in `KINDS` (`src/diagram/mod.rs:48`, private to that module). The first word of the header picks the kind.
- Callers outside `src/diagram/` name no kind. Use `diagram::validate` for live checks.
- Validation returns each problem with a byte `range` into the text. `import_failure` turns that range into a line number.
- `Error` marks a line the kind cannot read. `Warning` marks an unsupported construct or an empty diagram.
- A `Fix` replaces only its diagnostic's `range`. Tab applies a fix in the `:mermaid` editor, and only when `quick_fixes` is on.
- A fix ships only when its fixed text still parses.
- Add a validation test in `tests/validate.rs` and an import test in `tests/import.rs`.

`src/diagram/mod.rs:62` picks the kind by header word, so the UI calls one function.

```rust
pub fn validate(text: &str) -> Vec<Diagnostic> {
```

- Good: `validate` above. A new kind joins `KINDS`, and the UI does not change.
- Bad: `src/export/mod.rs:17`, `src/app/import.rs:15` and `src/cli.rs:251` name `Flowchart` directly. A second kind needs changes in each. Add a dispatch function in `src/diagram/mod.rs` before you add a second kind.

A fix must change only the broken part. `src/diagram/flowchart/validate.rs:37` keeps a candidate only when `parse_line` accepts the fixed line.

```rust
.find(|(.., fixed)| parse_line(fixed).is_ok())
```

- Good: the `find` above drops a candidate that leaves the line unreadable.
- Bad: before commit `4fb9b6a`, `replace_arrow` rewrote the `->` inside `-->`. `A --> B -> C` became unparsable, so the line got no fix at all. Commit `4fb9b6a` added the skip for the tail of `-->`.

## Performance

`make perf` checks the frame and drag-step budgets, including frames at 25% and 300% zoom. A frame takes at most 16 ms (about 60 fps), and so does one drag step plus its frame. No test checks the other budgets yet. CI runs `make check` with `CLIDRAW_PERF_SLACK=4`. That value divides each measured median by 4. Local runs leave `CLIDRAW_PERF_SLACK` unset.

The budgets are in the [Performance table](README.md#performance).

`tests/perf.rs:177-185` reads `CLIDRAW_PERF_SLACK` and divides the median by it.

```rust
fn median(mut v: Vec<Duration>) -> Duration {
    v.sort();
    let slack = std::env::var("CLIDRAW_PERF_SLACK")
        .ok()
        .and_then(|s| s.parse::<u32>().ok())
        .filter(|n| *n > 0)
        .unwrap_or(1);
    v[v.len() / 2] / slack
}
```

- Good: the `median` helper above.
- Bad: n/a: no violation in the code.

### Terminal frame times

`bench` times each event from input to the flushed frame, in your real terminal. It measures the write into the terminal, not the terminal's own rendering. It uses the current terminal size, at least 40x16, and does not touch your canvases. Times are in ms.

| Terminal | drag median | drag p99 | drag worst | pan median | pan p99 | pan worst | B/frame |
| --- | --- | --- | --- | --- | --- | --- | --- |
| kitty | not run yet | | | | | | |
| Alacritty | not run yet | | | | | | |
| Ghostty | not run yet | | | | | | |
| Rex | not run yet | | | | | | |

### Profile

Install the profiler once. It is a dev tool, not a project dependency: `cargo install samply`.

Run `make profile`. It builds the perf test with debug symbols in release, then records the drag scenario (`busy_app`, 5000 elements) with samply. samply opens the Firefox Profiler view in a browser. Use that view as the flamegraph.

The samply step is not verified yet: the cargo build runs, but `make profile` was not run to completion.

## UI design system

Every change to the chrome follows these rules. The chrome is the bars, the option strip, the overlays and the status line. Code that predates a rule migrates through the `ui-chrome` backlog epic. Do not copy a pattern that breaks a rule. Fix the pattern.

### Screen layout

```
row 0       tool row      Select v  Rect r  Diamond d  …  Text t
row 1       option strip  ‹ -> end <- start │ __ solid -- dashed │ ink › fill ›  ›   (only with a selection)
rows 1|2…   canvas viewport
last row    bottom bar    [Undo] [Redo]  MODE  key action  key action …   12x4  3 sel  [-] 100% [+] [Fit]  demo ●
```

- The chrome reserves its rows. The canvas viewport, Fit all, the minimap, glides and hit testing use only the rows between the bars. Nothing on the canvas hides under the chrome.
- The tool row and the option strip are one row each. They never wrap. When the content overflows, the row scrolls horizontally: the horizontal wheel or Shift-wheel scrolls it, and clickable `‹` `›` cells at the edges show that more content exists.
- The option strip separates its groups (head, stroke, border, align, color, placement, group) with `│`.
- Bottom bar, left zone: `[Undo] [Redo]`, the mode badge, then key hints for the current mode. A status message replaces the hints only, never the buttons or the right zone.
- Bottom bar, right zone, in this order: drag readout, selection count, `[-] <zoom> [+] [Fit]`, canvas name. An unsaved canvas shows ` ●` after its name.
- Narrow screens drop items in this order: hints (last hint first), drag readout, selection count. The buttons stay. The canvas name truncates with `…`. If the buttons, the zoom cluster and a 2-column name do not fit, the mode badge drops too.
- The `:` command line and the `/` find input replace the bottom bar, as in vim. Completions show in a list above the bar.
- Centered dialogs are only for input that needs a full line: export path, new canvas, rename, delete confirmation.
- Present mode draws the canvas only, with no chrome and no hint.

### Overlays

- Every list overlay uses the shared list component: the context menu, export scope, canvas switcher, actions palette, help and command completions.
- Box: `boxed()` with the title in the top border. Content starts 2 columns from the border. The box holds no hint text, because hints live in the bottom bar.
- Keyboard: Up/Down and Ctrl-p/Ctrl-n move. Enter runs. Esc clears a non-empty query first, then closes. A list without a query also takes j/k.
- Mouse: a click on a row runs it. A click outside the box closes it.
- A row shows its label on the left and its key on the right in the muted style.
- An open overlay dims everything under it.
- Every action that the mouse can reach is also reachable by keyboard: a shortcut, a palette entry, or both.

`src/ui/mod.rs:150` defines the box helper that every overlay uses.

```rust
pub(crate) fn boxed(grid: &mut Grid, (x, y, w, h): (i32, i32, i32, i32), title: &str) {
```

- Good: `boxed` draws the frame and the title.
- Bad: n/a: no violation in the code.

### Style tokens

Chrome code uses the named styles in the UI style module, never raw `BOLD`, `DIM`, `REVERSE` or `UNDERLINE` flags.

| Token | Use | Rendering |
| --- | --- | --- |
| key | a key in a hint or a list row | bold |
| muted | an action word, secondary text, a key column | dim |
| active | the current tool, the selected row, an active option, the mode badge | accent color; reverse without truecolor or with `NO_COLOR` |
| hover | the cell under the pointer | underline |
| disabled | an action that cannot run now | dim |
| error | an error status | reverse |

- The `chrome_accent` setting in `config.toml` sets the accent color.
- Meaning never depends on color alone. Every state also has a glyph, a label or reverse video.

The tokens live in `src/ui/style.rs`.

- Good: `src/ui/list.rs:199` passes the `MUTED` token for the key column:

  ```rust
  if on { ACTIVE } else { MUTED },
  ```

- Bad: `grid.text(x, y, label, DIM);` passes a raw flag.

### Spacing

- Bar items: 2 columns apart (`GAP`).
- Clickable cells: 1 column of padding on each side.
- Measure every width with `unicode-width`, never with `str::len` or `chars().count()`.

- Good: `src/ui/infobar.rs:7` names the gap, and `src/ui/infobar.rs:106` uses it.

  ```rust
  const GAP: i32 = 2;
  ```

  ```rust
  x -= width(&t) + GAP;
  ```

- Bad: `src/ui/prompt.rs:65` measures the button with `len`, which counts bytes, not cells:

  ```rust
  let ok = x + 2..x + 2 + OK.len() as i32;
  ```

- Good width: `src/model/geom.rs:147` asks `unicode-width` for the cell width of a char.

  ```rust
  let cw = c.width().unwrap_or(0);
  ```

- Bad width: `src/ui/perf.rs:43` counts code points, and a wide glyph takes two cells.

  ```rust
  let x = (grid.w - text.chars().count() as i32).max(0);
  ```

### Copy

- Key notation: `Ctrl-a`, `Shift-arrows`, `Ctrl-wheel`, `Alt-Enter`, `Esc`, `Enter`, `Del`, `Up/Down`. Use one notation everywhere.
- One key registry defines every key, its label and its scope. Hints, help, the palette and tooltips read it. Never write a key label by hand in a second place.
- Labels use sentence case and start with a verb when they name an action: `Export SVG`, `Bring to front`.
- Palette labels for a family use a category prefix: `Tool: Rect`, `Stroke: dashed`.
- A status message has no trailing period and starts with a capital letter.
- An error names what failed and how to recover: `Nothing selected: select a shape first`, `Usage: :e <name>`.

- Key notation, good: `src/ui/registry.rs` renders every label with `Key::label`.

  ```rust
  Binding::new("redo", &[ctrl('r')], "Redo", Scope::Canvas, Some(Action::Ex("redo"))),
  ```

- Key notation, bad: n/a: no violation in the code.

- Error status, good: `src/app/command.rs:24` names the usage.

  ```rust
  "e" if arg.trim().is_empty() => self.status = Some(Status::error("Usage: :e <name>")),
  ```

- Error status, bad: `src/app/export.rs:20` starts with a lowercase word and gives no recovery step.

  ```rust
  self.status = Some(Status::error("nothing selected"))
  ```

### Tests

- A chrome change gets a render test: render the screen into a buffer and assert the row text.
- Test each changed bar at 40, 60 and 120 columns.

`tests/render.rs:69-81` renders the screen into a `TestBackend`, and `row` reads the text of one row.

```rust
fn render(s: &Session, overlay: &Overlay) -> Buffer {
    let mut t = Terminal::new(TestBackend::new(80, 24)).unwrap();
    t.draw(|f| {
        let area = f.area();
        ui::render(f.buffer_mut(), area, s, overlay, None, None, &doc(true))
    })
    .unwrap();
    t.backend().buffer().clone()
}

fn row(b: &Buffer, y: u16) -> String {
    (0..80).map(|x| b[(x, y)].symbol().to_string()).collect()
}
```

- Good: the `render` and `row` helpers above.
- Bad: n/a: no violation in the code.

## Pull requests

Run `make check` before you open a pull request. CI runs the same check.
