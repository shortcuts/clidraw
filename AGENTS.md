# AGENTS.md

clidraw is a Rust terminal whiteboard which allows exporting to SVG or Mermaid.

## Breaking changes

clidraw is in alpha. Do not consider backward compatibility, deadcode, record a save-format break in the backlog, and do not add migrations. Making the app work comes first.

## Documentation

The code is the source of truth. Documentation describes the code and never replaces it.
- README.md: short. It states what clidraw is and redirects to the site at https://clidraw.shrtcts.fr. Review it with the `documentation` and `unslop` skills.
- docs/wiki/: the user documentation site on GitHub Pages. It holds the guides and the reference pages. Follow [docs/wiki/CONTRIBUTING.md](docs/wiki/CONTRIBUTING.md) for every page.
- docs/wiki reference pages: generated from the code with `UPDATE_DOCS=1`. Commit the generated output.
- CONTRIBUTING.md: contributor rules only. Put user-facing content in docs/wiki.

## Contributing

Read [CONTRIBUTING.md](CONTRIBUTING.md) for the constraints, test rules, performance workflow and UI design system. Follow it for every change.

## Live checks

The maintainer runs every `[Live]` plan check by hand. Never ask for one, never wait for one, and never block or mark a task inconclusive because one did not run. Run every other check, then report the task done.

To see the real TUI yourself, write a step file and run `DRIVE=<file> cargo test --test drive -- --ignored --nocapture`. It drives the binary in a pty with keys, clicks and drags, and prints the screen. A new regression check goes in `tests/e2e.rs` instead. A red test you did not cause is not a reason to stop: fix it or log it with `/radin-record`, then report the task done.
