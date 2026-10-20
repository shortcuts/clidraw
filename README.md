# clidraw

A full-screen terminal whiteboard. Draw with the mouse on an infinite canvas.

## Run

- `make run` opens the last canvas, or creates `untitled`.
- `cargo run -- <name>` opens or creates the canvas `<name>`.
- `cargo run -- config` prints the default `config.toml`. A canvas named `config` opens from the menu, not by argument.
- `cargo run -- probe` prints every terminal event. Use it to test a terminal.

The terminal needs true color, SGR mouse reporting and the alternate screen.

## Use

- Pick a tool in the palette: Select, Hand, Rect, Ellipse, Diamond, Line, Arrow, Text.
- Drag on empty canvas to draw. Drag a selected element to move it.
- Drag a corner handle to resize. Drag a line end onto a shape to bind it.
- Double-click a shape or line to edit its label. Double-click empty canvas to place text.
- Click a shape tool while a shape is selected to change its kind.
- Select a line to show its options in the palette. Pick a head: none, end, start or both.
- Drag the `::` cell of the palette to dock it to another edge.
- Wheel and horizontal scroll pan the canvas. A fast flick pans further. Shift+wheel pans sideways.
- Dragging within 2 cells of the screen edge pans the view.

| Key | Action |
|---|---|
| `Esc` | Cancel the gesture, else open or close the menu |
| `Delete` / `Backspace` | Delete the selection |
| `]` / `[` | Raise / lower the selection one step |
| `}` / `{` | Send the selection to the front / back |
| `Ctrl-z` / `Ctrl-y` | Undo / redo |
| `Ctrl-c` | Quit |
| `?` | Show the shortcuts, one page per scope |
| `:` | Open the command line: `q`, `w`, `wq`, `e <name>`, `u`, `redo`, `perf` |

The menu has Canvases, Fit all, Export SVG, Export Mermaid and Quit. Saving is automatic.

Mouse capture disables the terminal's own text selection. Hold Shift (Option on macOS) to select text.

## Render Mermaid as SVG

The built-in SVG export draws plain shapes. For a Mermaid render, export Mermaid and render it with [pele](https://pele.run). pele needs Node.

1. Choose Export Mermaid in the menu and enter a file path, such as `diagram.mmd`.
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

`make perf` checks the frame and drag-step budgets. A frame takes at most 16 ms (about 60 fps), and so does one drag step plus its frame. No test checks the other budgets yet. CI runs `make check` with `CLIDRAW_PERF_SLACK=4`, which divides each measured median by 4; local runs leave it unset.

### Profile

Install the profiler once. It is a dev tool, not a project dependency: `cargo install samply`.

Run `make profile`. It builds the perf test with debug symbols in release, then records the drag scenario (`busy_app`, 5000 elements) with samply. samply opens the Firefox Profiler view in a browser. Use that view as the flamegraph.

The samply step is not verified yet: the cargo build runs, but `make profile` was not run to completion.

## Files

- Canvases: `$CLIDRAW_HOME`, else `$XDG_CONFIG_HOME/clidraw`, else `~/.config/clidraw`. One `<name>.clidraw.json` per canvas. Output is stable, so the files diff cleanly in git.
- Settings: `config.toml` in the canvas directory. The first run writes it with every key commented. Keys: `autosave_ms` (500), `double_click_ms` (400), `dock` (`left`, `top`, `right`, `bottom`; default `left`), `default_canvas` (`untitled`). A bad or unknown key shows a status line and keeps the default.
- Viewport and last canvas: `$XDG_STATE_HOME/clidraw/state.json`, else `~/.local/state/clidraw/state.json`.
