# clidraw

A full-screen terminal whiteboard. Draw with the mouse on an infinite canvas.

## Run

- `make run` opens the last canvas, or creates `untitled`.
- `cargo run -- <name>` opens or creates the canvas `<name>`.
- `cargo run -- config` prints the default `config.toml`. A canvas named `config` opens from the canvas switcher (Ctrl-p, Canvases), not by argument.
- `cargo run -- list|new|show|edit|import|export` run without the UI, for scripts and agents. Each prints JSON on stdout, or the export text, and exits 1 with `Error: <msg>` on stderr. A canvas named like one of these commands opens from the canvas switcher (Ctrl-p, Canvases), not by argument.
  - `list` prints the canvas names as a JSON array. `new <name>` creates an empty canvas. `show <name>` prints the canvas file.
  - `edit <name>` reads a JSON array of ops on stdin and applies them all or none. It prints `{"applied":N,"ids":[...]}` with the ids of the created elements. The canvas must exist.
  - `import <name> <path|->` adds a Mermaid flowchart below the existing content and creates the canvas if missing.
  - `export <name> --svg|--mermaid|--text <path|->` writes a saved canvas to a file, or to stdout for `-`.
  - Ops: `add_shape` (`x`, `y`, optional `kind`, `w`, `h`, `label`, `color`, `fill`, `id`), `add_text` (`x`, `y`, `text`, optional `color`, `id`), `connect` (`from`, `to`, optional `kind`, `label`, `id`), `set` (`id`, optional `label`, `color`, `fill`), `move` (`id`, `dx`, `dy`), `delete` (`id`). Shape `kind` is `Rect`, `Diamond`, `Cylinder`, `Parallelogram`, `Hexagon` or `Frame`. Line `kind` is `Line`, `Arrow`, `ArrowStart` or `ArrowBoth`. Colors are `red`, `orange`, `yellow`, `green`, `blue`, `violet`, or `none` in `set`. A `connect` end is a shape id or an `[x, y]` cell.

    ```sh
    echo '[{"op":"add_shape","id":"a","x":0,"y":0,"label":"Hi"},{"op":"add_shape","id":"b","x":20,"y":0,"label":"Yo"},{"op":"connect","from":"a","to":"b"}]' | clidraw edit demo
    ```
- `cargo run -- mcp` serves the six commands above as MCP tools (`list`, `new`, `show`, `edit`, `import`, `export`) over stdio, for agent clients. Config: `{"mcpServers":{"clidraw":{"command":"clidraw","args":["mcp"]}}}`. A tool failure comes back as a result with `isError` true. A running UI overwrites an edit made to the canvas it has open. A canvas named `mcp` opens from the canvas switcher (Ctrl-p, Canvases), not by argument.
- `cargo run -- probe` prints the kitty keyboard support result, then every terminal event. Use it to test a terminal.
- `cargo run --release -- bench` replays a scripted drag and pan on a generated 5000-element canvas in your terminal and prints the frame times.
- `cargo test --test e2e` runs the real binary in a pty and asserts on the parsed screen. Use it to prove a Live box without a human.

The terminal needs true color, SGR mouse reporting and the alternate screen.

## Use

- Pick a tool in the top bar: Select, Rect, Diamond, Cylinder, Slant, Hexagon, Frame (`f`), Arrow, Text.
- Draw a frame around shapes, then click its border to move them together.
- Drag on empty canvas to pan, or to draw with a shape tool. Drag a selected element to move it.
- Moving shapes snaps to other shapes' edges and centers within 1 cell and shows dotted guides. Hold Shift to turn guides off. Guides override `grid`.
- Drag a selected line to bend it. Drag a bend handle to move it. Right-click the line and choose Reset route to go back to the automatic route.
- Drag the `⤡` handle at the bottom-right corner to resize. Drag a line end onto a shape to bind it: the shape turns bold and `◆` marks the cell where the end will attach.
- Double-click a shape or line to edit its label. Double-click empty canvas to place text.
- In a shape label, Alt-Enter or Ctrl-j starts a new line. Long lines wrap and the shape grows to fit.
- Select shapes and run `:layout` (top-down) or `:layout lr` to arrange them by their connecting lines.
- Run `:import <path>` to add a Mermaid flowchart to the canvas. It reads nodes, edges and labels and arranges them with `:layout`.
- Select shapes and run `:lib save <name>` to keep them in the library. `:lib insert <name>` adds them to any canvas.
- Group elements to select and move them as one.
- Click a shape tool while a shape is selected to change its kind.
- Select a line to show its options in the top bar. Pick a head: none, end, start or both.
- Select elements to get align, group and ungroup cells in the top bar. Each shows its key.
- Select a shape and click the align cell in the top bar to cycle its label: left, middle, right.
- Wheel and horizontal scroll pan the canvas. A fast flick pans further. Shift+wheel pans sideways.
- Dragging within 2 cells of the screen edge pans the view.
- A map shows in a corner while content lies off screen. The outlined rectangle marks the view. Click the map to center the view there.
- Right-click an element or empty canvas for a menu: edit label, duplicate, delete, bring to front, raise, lower and send to back, or paste, select all and fit all.

| Key | Action |
|---|---|
| `Esc` | Cancel the gesture, else clear the selection |
| `Delete` / `Backspace` | Delete the selection |
| `<` `|` `>` `^` `-` `_` | Align the selection left, center, right, top, middle, bottom |
| `=` / `#` | Distribute the selection evenly, horizontally / vertically |
| `g` / `G` | Group, ungroup the selection |
| `]` / `[` | Raise / lower the selection one step |
| `}` / `{` | Send the selection to the front / back |
| `Ctrl-b` | Toggle bold on the labels of the selected shapes |
| `y` / `x` / `p` | Copy / cut / paste the selection |
| `u` / `Ctrl-r` | Undo / redo |
| `z` | Fit all: the largest zoom up to 1x that shows the whole diagram |
| Ctrl + wheel | Zoom 25%, 50%, 100%, 200%, 300% around the cursor; 25% and 50% are read-only, `Esc` returns to 100% |
| Bottom bar buttons | `[-]` `[+]` zoom, `[Fit]`, `[Undo]`, `[Redo]`; shown at 60 columns or more |
| Present (palette only) | Hides the top bar and status line; wheel scrolls; `Esc` returns |
| `/` | Find a label: type the text, `Enter` selects and centers the match; repeat to cycle (`:/text` does the same) |
| `Ctrl-c` | Quit |
| `Ctrl-p` | Open the command palette: type to filter every action, `Enter` runs it |
| `?` | Show the shortcuts, one page per scope; type to filter all pages |
| `:` | Open the command line: `q`, `w`, `wq`, `e <name>`, `u`, `redo`, `layout [td|lr]`, `import <path>`, `lib save|insert <name>`, `perf` |

The command palette (`Ctrl-p`) lists every action with its key, including Canvases, Fit all, the exports and Quit. Saving is automatic. The Minimap action shows the whole canvas; click a frame or shape to jump to it.

Export asks what to write: the whole canvas, a dragged area or the selection. Export Text (or `:export`) writes the canvas as plain text, as the screen draws it. Set `export_padding` in `config.toml` for the SVG margin in cells.

Mouse capture disables the terminal's own text selection. Hold Shift (Option on macOS) to select text.

## Scope

clidraw takes its core model from [excalidraw](https://excalidraw.com) and [tldraw](https://tldraw.com). It does not match either one feature for feature.

### Covered

- Shapes: rectangle, diamond, slant, hexagon, cylinder and frame.
- Lines and arrows that bind to shapes. A bound end follows its shape when the shape moves.
- Frames that move their content together.
- Marquee select (Shift+drag on empty canvas), multi-select drag, group and ungroup.
- Align, distribute and z-order commands.
- Snapping to shape edges and centers, and a grid.
- Labels with line breaks, word wrap, bold and alignment.
- Undo and redo, kept across sessions.
- Find by label, a command palette and a full-screen minimap.
- Auto layout, and Mermaid flowchart import.
- A library of reusable snippets.
- Shape links, exported to SVG and Mermaid.
- Export to SVG, Mermaid and plain text.

### Not covered

- Images, freehand drawing, sticky notes, video and embeds.

### Left out on purpose

- Live collaboration and shared canvases. Each canvas is one local file.
- Accounts and cloud storage.

## Render Mermaid as SVG

The built-in SVG export draws plain shapes. For a Mermaid render, export Mermaid and render it with [pele](https://pele.run). pele needs Node.

1. Choose Export Mermaid in the command palette and enter a file path, such as `diagram.mmd`.
2. Install pele in a folder of your choice: `npm install pele`
3. Save this script as `render.mjs` in the same folder:

   ```js
   import { readFileSync, writeFileSync } from 'node:fs';
   import { render } from 'pele';

   const { svg } = render(readFileSync(process.argv[2], 'utf8'));
   writeFileSync(process.argv[3], svg);
   ```

4. Render the diagram: `node render.mjs diagram.mmd diagram.svg`

pele has no command-line tool. The script calls its `render()` function.

## Performance

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

The release binary (`make release`) is 2,615,488 bytes (2.6 MB) on macOS x86_64, measured 2026-10-23. It has no size budget yet.

The loop wakes every 16 ms only while a Fit all or search glide runs.

`make perf` checks the frame and drag-step budgets, including frames at 25% and 300% zoom. A frame takes at most 16 ms (about 60 fps), and so does one drag step plus its frame. No test checks the other budgets yet. CI runs `make check` with `CLIDRAW_PERF_SLACK=4`, which divides each measured median by 4; local runs leave it unset.

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

## Contributing

Read [AGENTS.md](AGENTS.md) for the constraints and test rules. Run `make check` before you open a pull request.

## Files

- Canvases: `$CLIDRAW_HOME`, else `$XDG_CONFIG_HOME/clidraw`, else `~/.config/clidraw`. One `<name>.clidraw.json` per canvas. Output is stable, so the files diff cleanly in git.
- Settings: `config.toml` in the canvas directory. The first run writes it with every key commented. Keys: `autosave_ms` (500), `double_click_ms` (400), `default_canvas` (`untitled`), `grid` (1 = no snapping; a larger number snaps drawn, moved and resized shapes to multiples of that many cells), `theme` (`auto`, `dark`, `light`; default `auto`; auto asks the terminal for its background and falls back to dark), `scroll_speed` (100 = percent of the default distance wheel and trackpad scrolling pans; 1 to 1000), `motion` (`true`; `false` makes Fit all and search jump snap instead of gliding over 120 ms). A bad or unknown key shows a status line and keeps the default.
- Viewport and last canvas: `$XDG_STATE_HOME/clidraw/state.json`, else `~/.local/state/clidraw/state.json`.
- Undo history: `history/<name>.json` beside `state.json`. It holds at most `history_limit` steps and is dropped when the canvas file changes outside clidraw. Redo does not persist.
- Edits from the CLI while the UI has the same canvas open are lost: the UI autosaves its own state over them. A CLI edit also drops that canvas's undo history.
