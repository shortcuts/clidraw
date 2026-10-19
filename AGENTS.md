# AGENTS.md

clidraw is a Rust terminal diagram editor. Read `BOOTSTRAP.md` for the design.

## Commands

| Command | Purpose |
|---|---|
| `make build` | Build debug binary |
| `make run` | Run the editor |
| `make test` | Run all tests |
| `make lint` | `cargo clippy` with warnings as errors |
| `make fmt` | Format code |
| `make check` | Format check, lint, test. Run before you finish. |

## Rules

- The model (`src/model/`) knows nothing about the terminal, ratatui or crossterm.
- Exporters (`src/export/`) read the model only. Never read the screen buffer.
- A line end is `Bound(ShapeId)` or `Free(x, y)`. Mermaid export uses only lines bound at both ends.
- Canvases save atomically (temp file, then rename) and never overwrite a file with a higher `version`.
- Canvas files are git-tracked by users. Output is deterministic: fixed field order, no timestamps, no viewport, random IDs kept stable. Volatile state goes in `state.json`.
- The canvas is infinite. Coordinates are signed. Never assume a size.
- Every edit goes through a model command, so undo and redo work.
- The editor is mouse-first. Keep keyboard shortcuts minimal (see `BOOTSTRAP.md`).
- Test mouse input with scripted `MouseEvent` sequences, not a real terminal.
- Restore the terminal on exit and on panic. A broken terminal is a bug.
- Add a dependency only when the standard library cannot do the job. State why in the PR.

## Style

- Run `make fmt` before commit.
- Comments explain why, never what.
- Keep modules small. Split a file past about 300 lines.
- Use `anyhow::Result` in `main.rs` and `io.rs`. Use typed errors inside `model/`.

## Tests

- Export changes need a golden-file update in `tests/golden/`. Review the diff by eye.
- A bug fix starts with a failing test.
