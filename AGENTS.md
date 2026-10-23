# AGENTS.md

clidraw is a Rust terminal diagram editor which allows exporting to SVG or Mermaid.

## Constraints

The tool must be minimal, fast, effective and work on any OS.

## Breaking changes

clidraw is in alpha. Do not consider backward compatibility, deadcode, record a save-format break in the backlog, and do not add migrations. Making the app work comes first.

## Documentation

The code is the source of truth, the markdown files serves a specific purpose:
- README.md: for the users of the tool, must be straight to the point and reviewed with the `documentation` and `unslop` skill
- CONTRIBUTING.md: for contributors and agents, must contain every implementation constraints with concrete example of good and bad implementations

## Tests

- Export changes need a golden-file update in `tests/golden/`. Review the diff by eye.
- A bug fix starts with a failing test.
- A change to rendering, hit testing, input handling or save passes `make perf`.
- A perf fix starts with a perf test that fails against the budget.
- The budgets are in the [Performance table](README.md#performance) of the README.

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
- Narrow screens drop items in this order: hints (last hint first), drag readout, selection count. The buttons stay. The canvas name truncates with `…` and never disappears.
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

### Spacing

- Bar items: 2 columns apart (`GAP`).
- Clickable cells: 1 column of padding on each side.
- Measure every width with `unicode-width`, never with `str::len` or `chars().count()`.

### Copy

- Key notation: `Ctrl-a`, `Shift-arrows`, `Ctrl-wheel`, `Alt-Enter`, `Esc`, `Enter`, `Del`, `Up/Down`. Use one notation everywhere.
- One key registry defines every key, its label and its scope. Hints, help, the palette and tooltips read it. Never write a key label by hand in a second place.
- Labels use sentence case and start with a verb when they name an action: `Export SVG`, `Bring to front`.
- Palette labels for a family use a category prefix: `Tool: Rect`, `Stroke: dashed`.
- A status message has no trailing period and starts with a capital letter.
- An error names what failed and how to recover: `Nothing selected: select a shape first`, `Usage: :e <name>`.

### Tests

- A chrome change gets a render test: render the screen into a buffer and assert the row text.
- Test each changed bar at 40, 60 and 120 columns.
