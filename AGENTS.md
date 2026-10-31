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

## Verifying your work

The repository owns its test harness. You verify your own work through it. Never wait for a human.

- Drive the real binary: write a step file and run `DRIVE=<file> cargo test --test drive -- --ignored --nocapture`. It plays keys, clicks, drags and the wheel in a pty and prints the screen. Steps are listed in `tests/drive.rs`.
- Keep a regression check: add it to `tests/e2e.rs`, built on `tests/e2e/harness.rs`. The harness already has `click`, `drag`, `double_click`, `right_click`, `wheel_down` and `find`. Never report "no mouse driver".
- A missing way to observe a behavior is a harness gap. Extend the harness in the same task, then verify.
- A test that is red before your change still blocks the gate. Fix it. If you cannot, record it with `/radin-record` as a fix for the next iteration, then report your task done.
- Check the fixed test against the base commit (`git stash`, run, `git stash pop`) to prove you did not cause the failure.
