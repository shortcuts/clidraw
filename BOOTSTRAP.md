# clidraw bootstrap

clidraw is a full-screen terminal whiteboard in the style of Excalidraw. The user
sketches diagrams on an infinite canvas with the mouse. It saves canvases to disk
and exports SVG and Mermaid.

## Goals

- Infinite canvas in the terminal, drawn and edited with the mouse.
- Shapes from a palette bar. Shapes move and resize.
- Lines and arrows connect shapes. Text goes in a shape, on a line, or anywhere.
- Several named canvases, saved automatically, switchable from inside the app.
- Export SVG and Mermaid.

## Non-goals (v1)

- Old terminals. Targets are listed below.
- Zoom. A terminal cell has a fixed size. Panning only.
- Hand-drawn (rough) rendering. Freehand strokes.
- Collaboration, web UI, mobile.
- Vim modes. Keyboard stays minimal.

## Terminal targets

Supported: kitty, Alacritty, Ghostty, libghostty hosts, Rex (superlogical), and any
modern terminal with the features below. Nothing else is tested.

Required features:

- True color.
- SGR mouse reporting (mode 1006) with drag (1002) and wheel events.
- Alternate screen and bracketed paste.

Use if present, never require:

- Synchronized output (mode 2026). It prevents tearing on full redraws.
  The [terminaltrove comparison](https://terminaltrove.com/compare/terminals/ghostty-vs-kitty/)
  lists it for kitty, Ghostty and Alacritty. Alacritty documents it
  ([escape_support.md](https://git.causa-arcana.com/PolytreeDE/alacritty/commits/branch/v0.14/docs/escape_support.md)).
- Kitty keyboard protocol. It reports `Ctrl-z` and `Esc` without ambiguity.
  Alacritty added it in 2023. crossterm exposes it as `PushKeyboardEnhancementFlags`.

Not verified in this session:

- Rex mouse and keyboard support. The Rex docs search returned nothing on it.
  Check the Rex docs or run milestone 0 inside Rex.
- Pixel mouse reporting (mode 1016). Decision: ignore it. Use cell coordinates.

## Stack

Versions checked on 2026-10-19 with `cargo search`.

| Concern | Choice | Reason |
|---|---|---|
| Language | Rust (stable) | One static binary per OS. |
| TUI | `ratatui` 0.30 | Layout and widgets. |
| Terminal I/O | `crossterm` 0.29 | Mouse and keys. |
| File format | `serde` + `serde_json` | Human-readable save files. |
| Storage paths | `directories` 6 | Home and state directories. |
| SVG export | Hand-written string builder | The shape set is small. |
| Errors | `anyhow` | Binary crate, no public API. |
| Text width | `unicode-width` 0.2 | Correct cell width for CJK and emoji. |
| Test temp dirs | `tempfile` 3 (dev) | Persistence tests. |

## Research findings

### Mouse events (crossterm 0.29)

[`MouseEventKind`](https://docs.rs/crossterm/latest/crossterm/event/enum.MouseEventKind.html)
has: `Down(button)`, `Up(button)`, `Drag(button)`, `Moved`, `ScrollUp`, `ScrollDown`,
`ScrollLeft`, `ScrollRight`.

- Some terminals do not report the button on `Up` and `Drag`. crossterm returns `Left`.
  Treat every `Drag` as a left drag.
- crossterm has no double-click event. Measure it with a timer (two `Down` events
  on the same cell within about 400 ms).
- Mouse capture disables the terminal's text selection. Users hold Shift
  (Option on macOS terminals) to select text. State this in the help text.
- `EnableMouseCapture` turns on modes 1000, 1002, 1003, 1015 and 1006. This comes from
  memory. Check the crate source.

### Rendering

- Do not use ratatui's `Canvas` widget. It maps floating-point coordinates to dots.
  Our shapes, hit-testing and SVG export all use integer cells, so draw into
  our own cell grid.
- Box-drawing characters: `┌─┐│└┘` for rect, `╭╮╰╯` for rounded shapes and ellipses.
- Wrap each frame in synchronized output when the terminal supports it.

## Architecture

The model is the source of truth. The TUI, the storage layer and the exporters
read it. Never export from the screen buffer.

```
src/
  main.rs          terminal setup, panic hook, event loop
  model/           Canvas, Element, commands, undo history
  input/           mouse state machine, hit-testing, minimal key map
  view.rs          viewport (pan offset), world <-> screen mapping
  ui/              canvas view, palette bar, esc menu, canvas switcher
  store.rs         canvas files: list, load, save, rename, delete
  export/
    svg.rs         Canvas -> SVG string
    mermaid.rs     Canvas -> Mermaid flowchart string
```

### Model

A `Canvas` holds a z-ordered list of `Element`. The last element draws on top.

| Element | Fields |
|---|---|
| `Shape` | `id`, `kind` (`Rect`, `Ellipse`, `Diamond`), `x`, `y`, `w`, `h`, `label` |
| `Line` | `id`, `kind` (`Line`, `Arrow`), `from: End`, `to: End`, `label` |
| `Text` | `id`, `x`, `y`, `text` (free text, no border) |

`End` is `Bound(ShapeId)` or `Free(x, y)`.

- A line starts and ends free or bound. Dragging an end onto a shape binds it.
  A bound end follows the shape when the shape moves.
- A `Line` label sits at the middle of the path.
- Deleting a shape turns its bound ends into `Free` ends at their last position.
- Coordinates are signed integers (`i32`) in terminal cells. The canvas is unbounded.
- SVG export scales x by 8 and y by 16 (cell aspect ratio is about 1:2).
- Every edit is a command on the model. Undo and redo replay commands.

### Viewport

- The viewport is a pan offset `(ox, oy)` in world cells. Screen = world - offset.
- Render only elements whose bounding box meets the viewport. A linear scan is enough
  for v1. Add a spatial index when a canvas passes about 5000 elements.
- Save the offset in the state file (see Persistence), not in the canvas file.
- "Fit all" in the Esc menu moves the offset to the bounding box of all elements.

## Mouse interaction

The palette selects a tool. The tool decides what a drag on empty canvas does.

| Tool | Drag on empty canvas | Click on element |
|---|---|---|
| Select (default) | Rubber-band selection | Select it |
| Hand | Pan the canvas | Pan |
| Rect, Ellipse, Diamond | Draw the shape. Press = corner, release = opposite corner. | Select it |
| Line, Arrow | Draw a line. Press = start, release = end. | Select it |
| Text | Not used | Click empty canvas: place free text and start editing |

Gestures in every tool:

| Gesture | Result |
|---|---|
| Wheel, `ScrollLeft`, `ScrollRight` | Pan the canvas |
| Drag the body of a selected element | Move it. Bound line ends follow. |
| Drag a corner handle of a selected shape | Resize it |
| Drag an end handle of a selected line | Move that end. Release on a shape to bind. |
| Double-click a shape or a line | Edit its label inline |
| Double-click empty canvas | Place free text and start editing |
| Click empty canvas | Clear selection |

After one placement, the tool returns to Select.

### Event handling

- State machine: `Idle`, `Pressed { target, origin }`, `Dragging { action }`.
  Move to `Dragging` when the cell changes after `Down`. This separates a click from a drag.
- Hit-test in this order: handle, element (topmost first), empty. A line hits on any
  cell of its path.
- Drain all pending events, then redraw once. Mode 1003 floods the loop with `Moved` events.
- Inline text edit: single-line first. `Enter` commits, `Esc` cancels.

### Palette bar

- Entries: Select, Hand, Rect, Ellipse, Diamond, Line, Arrow, Text.
- Docked to the left edge by default.
- Drag the bar's title cell to move it. It snaps to the nearest edge on release.
- The active tool shows reversed colors.

### Esc menu

`Esc` opens a centered menu. Click an entry, or press `Esc` again to close.

- Canvases (opens the switcher)
- Fit all
- Export SVG
- Export Mermaid
- Quit

### Keyboard

| Key | Action |
|---|---|
| `Esc` | Open or close the menu, cancel the current gesture |
| `Delete` / `Backspace` | Delete the selection |
| `Ctrl-z` / `Ctrl-y` | Undo / redo |
| `Ctrl-c` | Quit |
| Text keys | Type into a label during inline edit |

There is no save key. Saving is automatic.

## Persistence

### Location

Users keep canvases in git, usually inside a dotfiles repo. So canvases live under the
config directory that dotfiles already manage. Resolve the directory in this order:

1. `$CLIDRAW_HOME`
2. `$XDG_CONFIG_HOME/clidraw`
3. `~/.config/clidraw` on macOS and Linux
4. `%APPDATA%\clidraw` on Windows

- macOS uses `~/.config`, not `~/Library/Application Support`. The `directories` crate
  would pick the Library path, so build the path by hand. Use `directories::BaseDirs`
  only to find the home directory.
- A user who symlinks `~/.config/clidraw` into a dotfiles repo needs no extra setup.
  Follow symlinks. Never replace a symlinked directory with a real one.

State that must not enter git goes elsewhere: `$XDG_STATE_HOME/clidraw/state.json`
(default `~/.local/state/clidraw/`; `%LOCALAPPDATA%\clidraw\` on Windows). It holds the
last opened canvas and each canvas's viewport offset.
Reason: a viewport stored in the canvas file would change the file on every pan and
pollute the git diff.

### Files

- One file per canvas: `<name>.clidraw.json`.
- Content: `version`, `elements`. No timestamps, no viewport, no other volatile field.
- The file has a top-level `"version": 1`. Refuse to open a higher version. Never overwrite it.
- Canvas names are file stems. Reject names with path separators.

Git-friendly output rules:

- Pretty-print with 2-space indent and a trailing newline.
- Serialize fields in a fixed order. Elements stay in z-order.
- One element per few lines, so a move changes only that element's lines.
- Element IDs are short random strings (8 characters) assigned at creation. Never
  renumber on save. A counter would conflict when two branches add elements.
- Do not write a file when the content is unchanged. This keeps mtimes and `git status` quiet.

### Saving

- Autosave 500 ms after the last edit, and on quit.
- Write atomically: write `.<name>.clidraw.json.tmp` in the same directory, then rename.
  A crash cannot leave a half-written canvas. The dot prefix keeps the temp file out of
  most editors' file lists. Delete stale temp files at startup.
- Save on panic through the panic hook.

### Canvas switcher

- List canvases by modified time. Click to open.
- Actions: New, Rename, Delete. Delete asks for confirmation.
- `clidraw` opens the last canvas. `clidraw <name>` opens or creates `<name>`.
- First launch creates a canvas named `untitled`.

## Export

- **SVG:** every element. Shapes become `<rect>`, `<ellipse>`, `<polygon>`. Lines become
  `<path>` with an arrow marker. Text becomes `<text>`. The viewBox is the bounding box
  of all elements plus a margin.
- **Mermaid:** a `flowchart` of shapes with both ends of a line bound. Free lines and free
  text have no Mermaid form. Skip them and show how many were skipped.
- Each export asks for a path. Default: `<canvas name>.svg` or `<canvas name>.mmd`.

## Open questions

- "Replace" for the palette. Readings:
  1. Click a palette entry while a shape is selected to change its kind.
  2. Drag a palette entry onto an existing shape.
  3. Right-click a shape and pick a new kind.
  Default: reading 1.
- Line path: a diagonal looks poor on a cell grid. Default: a one-bend orthogonal path.
  Routing around shapes stays out of v1.
- Rex capabilities (see Terminal targets).

## Milestones

0. **Mouse probe.** A small binary that enables mouse capture and prints each event.
   Run it in kitty, Alacritty, Ghostty and Rex. Record which report `Drag`, the `Up`
   button, `Moved`, and synchronized output. Add the result to this file.
1. **Skeleton.** Alternate screen, raw mode, event loop, terminal restored on exit and panic.
2. **Canvas and viewport.** Pan with the wheel. Draw a rect with a drag.
3. **Edit.** Select, move, resize, delete, undo and redo.
4. **Palette bar.** Tool switching, dock and drag.
5. **Lines.** Line and Arrow tools, free and bound ends, follow on move.
6. **Text.** Labels on shapes and lines, free text, inline edit.
7. **Persistence.** Store, autosave, atomic write, canvas switcher, `clidraw <name>`.
8. **Exports.** SVG, then Mermaid. Golden-file tests for both.

## Test plan

- Model: unit tests for commands, undo and redo, binding and unbinding of line ends.
- Input: feed scripted `MouseEvent` sequences to the state machine. Assert on the model.
- Store: `tempfile` directory plus `CLIDRAW_HOME`. Test round-trip, atomic write,
  higher-version refusal, bad canvas names, symlinked directory.
- Git stability: saving an unchanged canvas leaves the file bytes identical. Panning
  does not change the canvas file. Moving one shape changes only that element's lines.
- Exports: golden files in `tests/golden/`. Compare output as text.
- Rendering: ratatui `TestBackend`, assert on the buffer.

## Risks

- Terminal mouse and keyboard support differs. Milestone 0 measures it.
- Inline text edit on a cell grid is fiddly. Start with a single-line label.
- A line label on a one-bend path needs a rule for the label position. Use the middle
  cell of the longest segment.
- The user cannot select text in the terminal while mouse capture is on.

## Pele

[pele](https://pele.run) renders Mermaid text to SVG. It is a JavaScript library and
needs Node. Do not use it for the SVG export: the model exports SVG directly.
Use it only to preview the Mermaid export in docs or CI.
