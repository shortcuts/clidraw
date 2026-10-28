# Contributing

This file holds the rules for contributors and agents. The code is the source of truth. Each rule that the code shows has a Good and a Bad example from this repo.

## Constraints

The tool must be minimal, fast, effective and work on any OS.

Canvas names must store on every OS. Good: `flow-v2`, `console`. Bad: `CON`, `a:b`, `x.`, because Windows rejects them.

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

A line whose ends moved by one delta reuses its previous route shifted by that delta, never unchecked. The shift counts the shape cells and earlier-line cells it meets and is refused when that costs more than the old route did.

```rust
let lines = self.line_hits(canvas, find, (a, b), &moved, me, &mut deps) * LINE;
```

- Good: `Router::around` shifts the dropped route and runs the search only when the shifted cells cross more than the old ones.
- Bad: shifting the route without the check, so a line runs through a shape the drag passed.

## Workflow

Run these commands from the repo root.

- `make check` runs `fmt-check`, `lint`, `test` and `perf`. CI runs it on Ubuntu, macOS and Windows.
- `make test` runs `cargo test`.
- `make lint` runs clippy with `-D warnings`.
- `make fmt` formats the code. `make fmt-check` only reports the differences.
- `make perf` checks the budgets. See [Performance](#performance).
- `make profile` records the drag scenario with samply. See [Profile](#profile).
- `make release` builds the release binary.
- `make screenshots` renders the VHS tapes in `docs/tapes` into `docs/wiki/assets`. It needs `vhs`. See [Screenshots](#screenshots).
- `cargo test --test e2e` runs the real binary in a pty and asserts on the parsed screen.

## Tests

A test protects a behavior of clidraw against a regression. It never checks how a library or an external tool behaves.

### Strategy

Pick the cheapest layer that can fail for the regression.

| Layer | Where | Use it for |
| --- | --- | --- |
| Unit | `#[cfg(test)]` in `src/`, or `tests/model.rs`, `tests/config.rs`, `tests/store.rs`, `tests/validate.rs`, `tests/import.rs` | Logic with no terminal: model commands, routing, parsing, validation, config, storage. Default choice. |
| Session and app | `tests/input.rs`, `tests/app.rs`, `tests/render.rs` | One `Session` or `App` driven by synthetic events, with a `TestBackend` buffer. Use it for a gesture, a key, an overlay or one drawn frame. |
| Golden | `tests/golden.rs` | The full text of an export. |
| CLI and MCP | `tests/cli.rs`, `tests/mcp.rs` | The command line and the stdio server contract, run as the real binary. |
| E2E | `tests/e2e.rs` | The real binary in a pty with a parsed screen. Use it for what only a real terminal shows: signals, raw-mode input, a multi-step scenario. |
| Perf | `tests/perf.rs` | The budgets. Run with `make perf`. |

- Prefer the unit layer. It runs in milliseconds and fails with a small cause.
- Add an e2e test only to assert one scenario that no lower layer can reach. Keep the number small: each costs about half a second.
- Each contract has one owner test at the lowest layer that can see it. Add a second test at another layer only for a failure the owner cannot reach.
- A bug fix adds one regression test at the owner layer. Do not replay the scenario at every layer it crosses.
- Do not add a seam (export, flag, wrapper) that only a test calls. Test at the real boundary.

### Why `tests/app.rs` is slow

Each `App::open` takes about 78 ms on macOS. Two `sync_all` calls in `write_durable` (`src/store.rs:82,87`) cause most of that time. The registry test `registry_runnable_entries_dispatch_their_action` opens 72 apps, so it takes about 5 s.

The syncs stay. A drawing file must survive a power loss. Do not remove them to speed up tests.

### Remove a test when

- It asserts only the OK path of a helper, with no credible regression.
- It restates the code: a copied table, or an expected value that the code under test produced.
- It checks that a dependency works (`serde`, `ratatui`, `crossterm`).
- Another test asserts the same thing. Keep the most specific one and merge the extra input into it.
- It mutates process state that parallel tests read, such as `HOME`. Put all such checks in one test.

Keep a test that has no assertion on content only when it guards a real panic: small sizes, wide characters, overflow.

### Coverage

Coverage finds untested code. It is not a target.

```sh
cargo install cargo-llvm-cov
rustup component add llvm-tools-preview
cargo llvm-cov --summary-only --ignore-filename-regex 'tests/'
```

- The e2e binary runs in a child process, so its coverage does not count.
- Read a low-coverage file before you add a test. Skip code that only draws or prints and that a lower layer already asserts.
- A test added for coverage alone is junk: delete it.

### Rules

- Export changes need a golden-file update in `tests/golden/`. Review the diff by eye.
- A bug fix starts with a failing test.
- A bug fix test fails before the fix: run it and see the failure first.
- A change to rendering, hit testing, input handling or save passes `make perf`.
- A perf fix starts with a perf test that fails against the budget.
- The budgets are in the [Performance section](#performance).

`check` in `tests/golden.rs` compares the whole export with its golden file. `UPDATE_GOLDEN` rewrites the file.

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
- Bad: `empty_canvas_exports_without_panicking` in `tests/golden.rs` asserts only that the output contains `<svg`. That line is fine for the empty canvas. A full export needs a golden file.

## Diagram kinds

A diagram kind is only a text import and export format. Each kind implements the `DiagramKind` trait in `src/diagram/mod.rs`. The canvas stays free-form: it draws any diagram, with or without a kind. Flowchart is the one kind that exists today.

- Add a kind in its own module under `src/diagram/`. Implement `keywords`, `import`, `export` and `validate`.
- Register the kind in `KINDS` (in `src/diagram/mod.rs`, private to that module). The first word of the header picks the kind.
- Callers outside `src/diagram/` name no kind. Use `diagram::validate` for live checks.
- Validation returns each problem with a byte `range` into the text. `import_failure` turns that range into a line number.
- `Error` marks a line the kind cannot read. `Warning` marks an unsupported construct or an empty diagram.
- A `Fix` replaces only its diagnostic's `range`. Tab applies a fix in the `:mermaid` editor, and only when `quick_fixes` is on.
- A fix ships only when its fixed text still parses.
- Add a validation test in `tests/validate.rs` and an import test in `tests/import.rs`.

`validate` in `src/diagram/mod.rs` picks the kind by header word, so the UI calls one function.

```rust
pub fn validate(text: &str) -> Vec<Diagnostic> {
```

- Good: `validate` above. A new kind joins `KINDS`, and the UI does not change.
- Bad: `render` in `src/export/mod.rs`, `App::import_text` in `src/app/import.rs` and `import` in `src/cli.rs` name `Flowchart` directly. A second kind needs changes in each. Add a dispatch function in `src/diagram/mod.rs` before you add a second kind.

A fix must change only the broken part. `arrow_fix` in `src/diagram/flowchart/validate.rs` keeps a candidate only when `parse_line` accepts the fixed line.

```rust
.find(|(.., fixed)| parse_line(fixed).is_ok())
```

- Good: the `find` above drops a candidate that leaves the line unreadable.
- Bad: before commit `4fb9b6a`, `replace_arrow` rewrote the `->` inside `-->`. `A --> B -> C` became unparsable, so the line got no fix at all. Commit `4fb9b6a` added the skip for the tail of `-->`.

## Performance

`make perf` checks the frame and drag-step budgets, including frames at 25% and 300% zoom. A frame takes at most 16 ms (about 60 fps), and so does one drag step plus its frame. No test checks the other budgets yet. CI runs `make check` with `CLIDRAW_PERF_SLACK=4`. That value divides each measured median by 4. Local runs leave `CLIDRAW_PERF_SLACK` unset.

`median` in `tests/perf.rs` reads `CLIDRAW_PERF_SLACK` and divides the median by it.

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

### Budgets

Budgets for a release build with 5000 elements on a 200x50 screen:

| Budget | Limit |
| --- | --- |
| Input to last byte written (key, click, drag step, wheel step) | 16 ms |
| Idle: no bytes written and no redraw while nothing changes | 0 bytes |
| Startup to first frame | 50 ms |
| Load or save of a canvas | 50 ms, never inside a drag |
| Export to SVG or Mermaid | 100 ms |
| Terminal output per frame for a one-cell change | under 4 KB |
| Peak resident memory, 5000 elements (measured once, perf test process) | 4.3 MB |
| Peak resident memory, 50000 elements (measured once, perf test process) | 14.1 MB |

The loop wakes every 16 ms only while a Fit all or search glide runs.

### Terminal frame times

`bench` times each event from input to the flushed frame, in your real terminal. It measures the write into the terminal, not the terminal's own rendering. It uses the current terminal size, at least 40x16, and does not touch your canvases. Times are in ms.

| Terminal | drag median | drag p99 | drag worst | pan median | pan p99 | pan worst | B/frame |
| --- | --- | --- | --- | --- | --- | --- | --- |
| kitty | not run: not installed | | | | | | |
| Alacritty | not run: not installed | | | | | | |
| Ghostty | not run: not installed | | | | | | |
| Rex | 13.8 to 14.6 | 30.1 to 54.0 | 30.8 to 59.4 | 1.0 | 8.2 to 12.5 | 11.0 to 15.2 | 420 drag, 837 pan |

The Rex row spans 3 runs at 80x24. The drag median sits near the 16 ms budget at that size.

### Startup

Time from process start to the first frame, in a pty that answers like a terminal (3 runs each, 80x24):

| Terminal reply | First frame |
| --- | --- |
| OSC 11, DA1 and the keyboard query | 17 to 31 ms |
| DA1 and the keyboard query, no OSC 11 | 17 to 20 ms |
| none (silent link) | 2.52 s |

A silent link waits `REPLY_TIMEOUT` (500 ms) for `theme::detect`, then 2 s for crossterm's `supports_keyboard_enhancement`.

### Profile

Install the profiler once. It is a dev tool, not a project dependency: `cargo install samply`.

Run `make profile`. It builds the perf test with debug symbols in release, then records the drag scenario (`busy_app`, 5000 elements) with samply. samply opens the Firefox Profiler view in a browser. Use that view as the flamegraph.

`make profile` was run to completion with samply 0.13.1: it records, then serves the profile at `http://127.0.0.1:3000` until Ctrl+C.

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
- The `:` command line (with `command_ui = "vim"`) and the `/` find input replace the bottom bar, as in vim. Completions show in a list above the bar.
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

`boxed` in `src/ui/mod.rs` defines the box helper that every overlay uses.

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

- Good: `List::draw` in `src/ui/list.rs` passes the `MUTED` token for the key column:

  ```rust
  if on { ACTIVE } else { MUTED },
  ```

- Bad: `grid.text(x, y, label, DIM);` passes a raw flag.

### Spacing

- Bar items: 2 columns apart (`GAP`).
- Clickable cells: 1 column of padding on each side.
- Measure every width with `unicode-width`, never with `str::len` or `chars().count()`.

- Good: `GAP` in `src/ui/infobar.rs` names the gap, and `layout` uses it.

  ```rust
  const GAP: i32 = 2;
  ```

  ```rust
  x -= width(&t) + GAP;
  ```

- Bad: `hit` in `src/ui/prompt.rs` measures the button with `len`, which counts bytes, not cells:

  ```rust
  let ok = x + 2..x + 2 + OK.len() as i32;
  ```

- Good width: `wrap` in `src/model/geom.rs` asks `unicode-width` for the cell width of a char.

  ```rust
  let cw = c.width().unwrap_or(0);
  ```

- Bad width: `chars().count()` counts code points, and a wide glyph takes two cells.

  ```rust
  let w = name.chars().count() as i32 + 2;
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

- Error status, good: `App::run_command` in `src/app/command.rs` names the usage.

  ```rust
  "e" if arg.trim().is_empty() => self.status = Some(Status::error("Usage: :e <name>")),
  ```

- Error status, bad: a message that starts with a lowercase word and gives no recovery step.

  ```rust
  self.status = Some(Status::error("nothing selected"))
  ```

### Tests

- A chrome change gets a render test: render the screen into a buffer and assert the row text.
- Test each changed bar at 40, 60 and 120 columns.

`render` in `tests/render.rs` renders the screen into a `TestBackend`, and `row` reads the text of one row.

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

## Screenshots

The tapes in `docs/tapes` drive the release binary by keys only, because VHS sends no mouse events. They seed canvases with `clidraw edit` and `clidraw import`. CI does not render them. Commit the output.

Every tape must run against a throwaway home, so a render never edits a real canvas.

- Good: `Type "export CLIDRAW_HOME=$(mktemp -d) XDG_STATE_HOME=$(mktemp -d)" Enter` inside `Hide`, as in `docs/tapes/canvas.tape`.
- Bad: a tape that runs `clidraw flow` with the real home, so the render opens and autosaves your own canvas `flow`.

## Pull requests

Run `make check` before you open a pull request. CI runs the same check.

## Releases

- Write commit messages in Conventional Commits form. release-please reads them to pick the version.
- release-please keeps one release PR open. It edits `CHANGELOG.md`, `Cargo.toml` and `Cargo.lock`.
- Merging the release PR tags `v<version>`, creates a GitHub release and publishes to crates.io.
- The tag starts `.github/workflows/release.yml`. [dist](https://axodotdev.github.io/cargo-dist/) builds the binaries and installers, attaches them to the release and pushes the formula to `shortcuts/homebrew-tap`.
- `release.yml` is generated. Edit `dist-workspace.toml`, then run `dist generate`.
- A tag created with `GITHUB_TOKEN` does not start other workflows. The secret `RELEASE_PLEASE_TOKEN` (a fine-grained PAT) lets the release-please tag start `release.yml`. The secret `HOMEBREW_TAP_TOKEN` (the same PAT) lets dist push to the tap. The PAT needs Contents and Pull requests read/write on `clidraw` and `homebrew-tap`.
- Do not edit `CHANGELOG.md`, `.release-please-manifest.json` or the `version` in `Cargo.toml` by hand.
