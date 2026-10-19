# AGENTS.md

clidraw is a Rust terminal diagram editor which allows exporting to SVG or Mermaid.

## Constraints

The tool must be minimal, fast, effective and work on any OS.

## Tests

- Export changes need a golden-file update in `tests/golden/`. Review the diff by eye.
- A bug fix starts with a failing test.
