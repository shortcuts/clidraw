# clidraw

A full-screen terminal whiteboard. Draw with the mouse on an infinite canvas.

## Run

- `make run` opens the last canvas, or creates `untitled`.
- `cargo run -- <name>` opens or creates the canvas `<name>`.
- `cargo run -- probe` prints every terminal event. Use it to test a terminal.

The terminal needs true color, SGR mouse reporting and the alternate screen.

## Use

- Pick a tool in the palette: Select, Hand, Rect, Ellipse, Diamond, Line, Arrow, Text.
- Drag on empty canvas to draw. Drag a selected element to move it.
- Drag a corner handle to resize. Drag a line end onto a shape to bind it.
- Double-click a shape or line to edit its label. Double-click empty canvas to place text.
- Click a shape tool while a shape is selected to change its kind.
- Select a line to show its options in the palette. Pick a head: none, end or start.
- Drag the `::` cell of the palette to dock it to another edge.
- Wheel and horizontal scroll pan the canvas.

| Key | Action |
|---|---|
| `Esc` | Cancel the gesture, else open or close the menu |
| `Delete` / `Backspace` | Delete the selection |
| `Ctrl-z` / `Ctrl-y` | Undo / redo |
| `Ctrl-c` | Quit |
| `?` | Show the shortcuts, one page per scope |

The menu has Canvases, Fit all, Export SVG, Export Mermaid and Quit. Saving is automatic.

Mouse capture disables the terminal's own text selection. Hold Shift (Option on macOS) to select text.

## Files

- Canvases: `$CLIDRAW_HOME`, else `$XDG_CONFIG_HOME/clidraw`, else `~/.config/clidraw`. One `<name>.clidraw.json` per canvas. Output is stable, so the files diff cleanly in git.
- Viewport and last canvas: `$XDG_STATE_HOME/clidraw/state.json`, else `~/.local/state/clidraw/state.json`.
