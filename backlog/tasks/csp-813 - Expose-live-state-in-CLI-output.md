---
id: CSP-813
title: Expose live state in CLI output
status: Done
assignee:
  - '@claude'
created_date: '2026-11-12 01:47'
updated_date: '2026-11-12 08:10'
labels:
  - live-state
  - ls-cli
milestone: m-26
dependencies:
  - CSP-812
  - CSP-811
documentation:
  - >-
    docs/adr/0205-live-state-is-a-snapshot-side-table-bound-to-sessions-and-muxes.md
priority: medium
type: enhancement
ordinal: 899000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Live state should be reachable from scripts and from `conspectus` commands before any TUI work, both to prove the data layer and because the machine-readable graph is a design requirement. The first slice also needs a way to see why a harness shows nothing.

Scope: `graph --format json` carries `node_live` (done by the side table; add a golden with state). `conspectus table` gets a `live` column for sessions and a counts column for muxes, off by default like `activity`, listed by `conspectus columns`. `node show` prints state, detail, since, age, quiet or stale, and source. `hook status` gains a health line per harness: events seen recently, or "active but no events: hooks may be untrusted, disabled, or not installed". The `--state` filter is left for the filtering story.

TUI changes: none visible. The TUI renders the same frames before and after (check a few views with `conspectus tui --snapshot`).

Tests and checks: Table and node-show goldens with a populated and an empty store, the `columns` listing test, health line for each of installed/active/silent.

Blockers: Binding.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [x] #1 `conspectus table --columns ...,live` shows the effective state with its detail and age
- [x] #2 `node show` on a session and on a mux prints the live fields
- [x] #3 `hook status` reports a health line per harness
- [x] #4 Goldens cover populated and empty stores
<!-- AC:END -->

## Definition of Done
<!-- DOD:BEGIN -->
- [x] #1 `just check` passes, or `git diff --check` for docs-only changes
- [x] #2 Docs and ADRs are updated when behavior, architecture, or workflow change
<!-- DOD:END -->

## Implementation Plan

<!-- SECTION:PLAN:BEGIN -->
1. Confirm `graph --format json` already carries `node_live` (golden `graph_snapshots__live_state` and the `hook write` end-to-end test); no change needed there.
2. Columns: add an opt-in `live` column to SESSIONS_COLUMNS and MUX_COLUMNS (default off, like `activity`), so `--columns`, `[table.<rows>].columns`, and `conspectus columns` pick it up. A crate-internal `output::live` module formats the cells through `live_state_of` / `live_counts_of_mux` at `RenderOptions::now_epoch` (the CLI's current clock; tests pin it): session `state[:detail] [+N] [quiet] <age>`, mux `1 waiting, 2 working` in default precedence, `—` without evidence.
3. node show: `render_node_show_at(snapshot, id, color, now_ms)`; a `live state` section after the summary for a session (state, detail, via subagent, since, age, last event, confidence, subagents, source, process) and for a mux (counts plus its pane-only entry); nothing without evidence.
4. hook status: a last stdout line `<health>\t<harness>\t<store path>\t<message>`: receiving (store entries: N sessions, newest event ago), silent (installed, no entries, a running agent of the harness in the cached graph.bin, pid checked in /proc), no-events, off (not installed: how to enable), unsupported (the harness's hook reports no live state; new `HarnessAdapter::reports_live_state`, Claude Code true). Reads two files, no process, no daemon.
5. Tests: unit tests for cells and the health decision; replay goldens for the sessions and mux tables with a populated and an empty scratch store at a fixed clock; node show goldens for a session, a mux, and no evidence; columns listing; cli_smoke health lines for installed+recent, installed+silent, not installed, unsupported.
6. Docs: operations.md (columns, node show, hook status), library-api.md (now_epoch anchors live cells), CHANGELOG, plan README if needed. Verify tui --snapshot frames byte-identical; checks.sh and run-tests.sh.
<!-- SECTION:PLAN:END -->

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
Landed in four commits: the `live` columns (13bf4de7), node show (34a90a04), the hook status health line (8c72e310), docs (f3fdea13).
`graph --format json` needed nothing: CSP-808's `graph_snapshots__live_state` golden and CSP-812's `hook write` end-to-end test already cover `node_live` with state.
Choices: the cell's no-evidence mark is `—` like every other table cell; ages count from `since_ms`, or from the last event for a state the read rules derived (`exited:killed`, `unknown:stale`); `+N` and `quiet` are omitted for exited and unknown; `node show` times are UTC RFC 3339; the health line is a fourth tab-separated stdout line after the install lines, with labels receiving / silent / no-events / off / unsupported; running agents come from main-agent runtime processes in `graph.bin`, each checked in `/proc`; new `HarnessAdapter::reports_live_state` (Claude Code true).
Validation: checks.sh exit 0 (fmt, clippy -D warnings, rustdoc -D warnings, diff --check); run-tests.sh 5178 passed, 1 failed (the known `discovery::forge` gh spawn flake, passed on rerun); `tui --snapshot` frames for nine views byte-identical before and after (frames.sh); manual `hook status` in a scratch home took 12 ms. tuikit is untouched, so tuikit-features and the separation rehearsal were not rerun.
Follow-ups: CSP-842 (name running agents without live state when others report); notes on CSP-815 and CSP-817 to flip `reports_live_state`.
<!-- SECTION:NOTES:END -->

## Final Summary

<!-- SECTION:FINAL_SUMMARY:BEGIN -->
Live state is now reachable from `conspectus` commands, with nothing new in the TUI.

- `conspectus table sessions|mux` has an opt-in `live` column (off by default like `activity`; `--columns +live`, `[table.<rows>].columns`, and `conspectus columns` all know it). A session's cell is its effective state read through `live_state_of` at `RenderOptions::now_epoch` (the CLI's clock): `waiting:permission +1 3m`, `working:generating quiet 12m`, `exited:killed 4m`, `unknown:stale 20m`, `—` without evidence. A mux's cell is `live_counts_of_mux`, most urgent first (`1 waiting, 1 working, 1 unknown, 1 exited`).
- `node show` gets a `live state` section for a session (state, detail, via subagent, recorded state when replaced, since, age, last event, confidence, subagents, source, process) and for a mux (counts plus its pane-only agent); none without evidence. `render_node_show_at` takes the read time.
- `hook status <harness>` ends with `<health>\t<harness>\t<live-state file>\t<message>`: receiving, silent (installed, a running agent in `graph.bin` checked in `/proc`, no events), no-events, off (with the `hook init` command), unsupported (`HarnessAdapter::reports_live_state`, Claude Code only). No process, no daemon.
- `graph --format json` already carried `node_live` with a golden from CSP-808.

Verified by unit tests (cells, node show lines, health decisions, running agents), goldens over a scratch store bound by real discovery (populated and empty, fixed clock) and over the fixture graph (session, mux, no evidence), CLI smoke tests (columns listing, flag and config selection, health for not installed, installed with events, installed and silent, unsupported), checks.sh, the full suite (one known forge flake, passed on rerun), and byte-identical `tui --snapshot` frames for nine views. Docs: operations guide, design note, library inventory, changelog.
<!-- SECTION:FINAL_SUMMARY:END -->
