---
name: release-candidate
description: |
  Go/no-go audit of main before merging the release-please PR: checks, CI, docs,
  code quality. Logs every failure to radin's backlog.
  Use for "release checks", "pre-release audit", "is this ready to ship?".
---

# Release Candidate Auditor

Merging the release-please PR tags `v<version>`, publishes to crates.io and starts the dist
build. A bad merge is not reversible. This audit runs on `main` before that merge.

Every failure becomes a backlog entry detailed enough to act on without re-running the
audit. Log each one with `/radin-record`.

Done when every step below has run and every failure it found is logged.

## Step 0: Baseline

The entry count detects net-new entries at the end:

```bash
radin backlog count
```

Resolve the last release tag. It bounds "since last release" everywhere below. No tag
exists before the first release, so fall back to the bootstrap commit:

```bash
git describe --tags --abbrev=0 2>/dev/null || jq -r '."bootstrap-sha"' release-please-config.json
```

## Step 1: Checks

Run on a clean `main` that matches `origin/main`. Each target is a hard gate. Run them
one at a time so each failure is logged with its own output:

```bash
make fmt-check
make lint
make test
make perf
make release
```

`make test` includes `tests/docs.rs`. A failure there means a generated wiki page is
stale: log it as a documentation gap, not a test failure.

Per failure, `/radin-record`:

> Log a fix: `make <target>` failed. Exit code <N>. Output: <last 30 lines>

## Step 2: CI

CI also runs Windows, which this machine cannot:

```bash
gh run list --branch main --limit 1 --json conclusion,headSha,url
```

Done when the latest run covers `git rev-parse HEAD` and its conclusion is `success`.
An older SHA or a failure: `/radin-record` it with the run URL and the failing job.

## Step 3: Documentation consistency

The code is the source of truth. Every user-facing change since the tag appears in
`docs/wiki/`, and in `README.md` when it changes what clidraw is.

Done when every feature area with source changes since the tag is accounted for:
documented, stale-doc logged, or marked not user-facing.

```bash
git diff --name-only <last-release-tag> HEAD -- src docs/wiki README.md
```

Group the changed sources by feature area (keys, shapes and arrows, export, import,
canvases, config, CLI, MCP, agents). Per area, read the wiki page and compare it with the
changed sources:

1. A page covers the area, and its behavior matches the code.
2. Pages follow `docs/wiki/CONTRIBUTING.md`: one content type, no source paths or Rust types.
3. `docs/wiki/llms.txt` and `docs/wiki/sitemap.xml` list every page.
4. `README.md` stays short and links to the site.
5. `AGENTS.md` and `CONTRIBUTING.md` match the Makefile targets and the release flow.

Per gap, `/radin-record`:

> Log a fix: documentation gap in <feature area>. Changed sources: <paths>.
> Gap: <what is missing or stale, specific enough that the writer knows what to write>.
> Doc file: <path>.

A page that covers the change passes. Skip cosmetic nits.

## Step 4: Intermediate verdict

Run `radin backlog count` and compare it to the Step 0 baseline. More entries: report the
❌ verdict below and stop. Same count: continue to Step 5.

## Step 5: Thermo-nuclear code quality review

Run `/radin-review`. It reviews and logs each finding itself:

> Review all commits since `<last-release-tag>` (`git diff <last-release-tag> HEAD`).
> Apply the full thermo-nuclear standards and log findings to the backlog.

Log only findings that meet the thermo-nuclear bar: structural regressions, missed
code-judo, spaghetti growth, bad abstractions, file-size explosions.

## Step 6: Release PR

Done when the open release PR is inspected:

```bash
gh pr list --search 'chore(main): release in:title' --json number,title,headRefName,url
gh pr view <PR#> --json body,mergeable,statusCheckRollup
```

1. The PR exists. None means no `feat` or `fix` commit landed since the tag: nothing to release.
2. The proposed version matches the commits. A `feat` bumps the minor, a `fix` the patch (`bump-minor-pre-major` is on).
3. The PR body lists every user-facing `feat` and `fix` since the tag.
4. The PR is mergeable and its checks pass.

Per defect, `/radin-record` with the PR URL. A wrong version is a bad commit message:
fix it at the source, never by editing `CHANGELOG.md`, `Cargo.toml` or the manifest.

## Step 7: Verdict

Run `radin backlog count` and compare it to the Step 0 baseline.

Same count:

> ✅ Release is ready. Format, lint, tests, perf, release build, CI, documentation
> consistency, thermo-nuclear review and release PR all passed. Merge the release-please PR.

More entries:

> ❌ Release is NOT ready. N issue(s) were logged to the backlog during this audit.
> Resolve them before merging the release PR.
>
> Issues logged:
> - [headings of the items added during this run]

## Notes

- Keep lint errors visible. A suppression that hides a real issue is itself a finding: log it.
- The maintainer runs `[Live]` checks by hand. Never ask for one or wait for one.
