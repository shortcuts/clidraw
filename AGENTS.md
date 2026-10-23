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
