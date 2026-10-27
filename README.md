# clidraw

[![CI](https://github.com/shortcuts/clidraw/actions/workflows/ci.yml/badge.svg)](https://github.com/shortcuts/clidraw/actions/workflows/ci.yml)
[![License](https://img.shields.io/github/license/shortcuts/clidraw)](LICENSE)
[![crates.io](https://img.shields.io/crates/v/clidraw)](https://crates.io/crates/clidraw)

![clidraw demo](https://raw.githubusercontent.com/shortcuts/clidraw/main/docs/wiki/assets/hero.gif)

A full-screen terminal whiteboard. Draw with the mouse on an infinite canvas.
Export to SVG or Mermaid, import Mermaid, and drive it from scripts and agents.

## Features

- Draw [flowcharts](https://clidraw.shrtcts.fr/draw-a-flowchart.html) with arrows that bind to shapes.
- [Import Mermaid](https://clidraw.shrtcts.fr/import-mermaid.html) and [export SVG](https://clidraw.shrtcts.fr/export-svg.html) or Mermaid.
- [Manage canvases](https://clidraw.shrtcts.fr/manage-canvases.html). Each canvas is one local file.
- Use the [command line](https://clidraw.shrtcts.fr/cli.html) or [MCP tools](https://clidraw.shrtcts.fr/mcp.html).
- Read the [scope](https://clidraw.shrtcts.fr/scope.html): what clidraw covers and what it leaves out.

## Install

- Shell: `curl --proto '=https' --tlsv1.2 -LsSf https://github.com/shortcuts/clidraw/releases/latest/download/clidraw-installer.sh | sh`
- PowerShell: `powershell -ExecutionPolicy Bypass -c "irm https://github.com/shortcuts/clidraw/releases/latest/download/clidraw-installer.ps1 | iex"`
- Homebrew: `brew install shortcuts/tap/clidraw`
- mise: `mise use -g github:shortcuts/clidraw`
- Cargo: `cargo install clidraw`

All methods and the terminal requirements are on the [install page](https://clidraw.shrtcts.fr/install.html#methods).

## Quick start

Run `clidraw first` to open or create the canvas `first`.
Pick a tool in the top bar, then drag on the canvas.
The [getting started guide](https://clidraw.shrtcts.fr/) covers the rest.

## Agents

Connect an AI agent to clidraw with the [agent guide](https://clidraw.shrtcts.fr/agents.html).

## Documentation

- [clidraw.shrtcts.fr](https://clidraw.shrtcts.fr/): guides, command reference and MCP tools.
- [Contributing](CONTRIBUTING.md): rules for contributors.
- [License](LICENSE): MIT.
- [Sponsor](https://github.com/sponsors/shortcuts).

## Before going public

TODO: remove this section once the repo is public.

- [ ] Enable GitHub Pages (source: GitHub Actions) and point the `clidraw.shrtcts.fr` CNAME in DNS at the Pages host.
- [ ] Set the repo homepage to `https://clidraw.shrtcts.fr`.
- [ ] Create the `shortcuts/homebrew-tap` repo.
- [ ] Create the PATs and set the secrets `RELEASE_PLEASE_TOKEN`, `HOMEBREW_TAP_TOKEN` and `CARGO_REGISTRY_TOKEN`.
- [ ] Enable "Allow GitHub Actions to create and approve pull requests".
- [ ] Make the repo public.

The browser live checks from the docs, release and install tasks run once `main` is pushed.
