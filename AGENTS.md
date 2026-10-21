# AGENTS.md

clidraw is a Rust terminal diagram editor which allows exporting to SVG or Mermaid.

## Constraints

The tool must be minimal, fast, effective and work on any OS.

clidraw is in alpha. Do not keep backward compatibility with older saved files. Each break needs a backlog entry that blocks the stable release candidate.

Non-rectangular shapes keep their size on load and import, even when it breaks their aspect ratio. The ratio applies only on create, resize, label growth and kind change.

## Tests

- Export changes need a golden-file update in `tests/golden/`. Review the diff by eye.
- A bug fix starts with a failing test.
- A change to rendering, hit testing, input handling or save passes `make perf`.
- A perf fix starts with a perf test that fails against the budget.
- The budgets are in the [Performance table](README.md#performance) of the README.
