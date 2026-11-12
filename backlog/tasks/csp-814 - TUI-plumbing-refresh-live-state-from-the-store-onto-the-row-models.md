---
id: CSP-814
title: 'TUI plumbing: refresh live state from the store onto the row models'
status: Done
assignee:
  - '@claude'
created_date: '2026-11-12 01:47'
updated_date: '2026-11-12 09:14'
labels:
  - live-state
  - ls-tui
milestone: m-26
dependencies:
  - CSP-812
documentation:
  - >-
    docs/adr/0205-live-state-is-a-snapshot-side-table-bound-to-sessions-and-muxes.md
  - docs/plans/live-state/research/06-internal-audit.md
priority: medium
type: enhancement
ordinal: 900000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
The TUI polls a whole snapshot every 30 seconds and has no push channel, so a hook event would take up to 30 seconds to show. The store is cheap to stat and read, so the TUI can re-bind it between snapshots with or without a daemon. This story builds that path and carries the data onto the row models without rendering anything, so the rendering decisions stay open.

Scope: In the runtime, stat the store once per tick and, when it changed and at least a second has passed, re-read it and re-run `bind_live` against the snapshot the app holds; hold the result as an overlay that the row builders read in place of the snapshot's table. Add a `live` field (effective state, detail, since, confidence, subagent count) to the session row, the mux row (as counts), and the mixed rows, filled for every view by shared code, not rendered. Handle the daemonless and daemon-backed refresh paths, a snapshot swap during an overlay refresh, and a missing store. Nothing reads it for display yet.

TUI changes: none visible: the data is on the row models but no column, glyph, or chip reads it. `conspectus tui --snapshot` frames are byte-identical before and after for the default views.

Tests and checks: Runtime tests for the stat/throttle logic and overlay refresh; consistency tests that the same entity carries the same live data in every view; the snapshot goldens unchanged; a test with a changing store file proving the overlay updates within the interval without a rediscovery.

Blockers: Binding.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [x] #1 A change to the store reaches the row models within about a second, with and without a daemon
- [x] #2 Session, mux, and mixed row models carry the live data, built by shared code for every view
- [x] #3 TUI snapshot output is unchanged for the default views
- [x] #4 A missing or corrupt store leaves rows without live data and raises no error
<!-- AC:END -->

## Definition of Done
<!-- DOD:BEGIN -->
- [x] #1 `just check` passes, or `git diff --check` for docs-only changes
- [x] #2 Docs and ADRs are updated when behavior, architecture, or workflow change
<!-- DOD:END -->

## Implementation Plan

<!-- SECTION:PLAN:BEGIN -->
1. Model: share the mux-count host rule (`live_counts_of_mux`) through a crate-private `live_counts_by_mux(table, now)` so the TUI counts muxes against its own table with the same semantics; give `LiveFile` a lenient text parser that `LiveStore::read` and the TUI share.
2. Row models: a TUI view-model in `tui::rows::live` (`LiveCell` for a session, `LiveCounts` for a mux, `MixedLive` for a mixed row) on `AgentSessionRow`, `MuxSessionRow`, and `MixedRow`, filled by one post-pass (`live::annotate`, the `content::annotate` pattern) that `build_tree_for_view` runs for every view with `now`; `TreeInputs` gains the live table. Builders set nothing themselves. Nothing renders it.
3. Overlay: `tui::live_overlay::LiveOverlay`, owned by the App, with a stat/read seam and a pid probe: one stat per tick, re-read only on a changed stamp and at least 1 s after the last read, re-bind with `bind_live` against the held snapshot; a missing or corrupt store is an empty table. Off (rows read the snapshot's own `node_live`) unless the live runtime turns it on. One runtime helper installs every new snapshot (worker result, daemon snapshot, `r`, static/fixture, snapshot mode) and re-binds the overlay before the tree is built. The App re-fills the live cells in place when the overlay's table changes or, at most once a second, when an effective state changed with time.
4. Tests: overlay unit tests (throttle, unchanged file never read, corrupt is empty, swap re-binds, no /proc on unchanged ticks); runtime rig tests (a store write reaches the row models without a discovery, with and without a daemon; a new snapshot re-binds); consistency test (one entity, one live value in every view); rendering regression (populated vs empty overlay, identical frames for every view); model test for the shared count rule.
5. Docs: design.md and the plan README note that the data is on the row models and rendering waits on the parked decisions.
6. Verify: `tui --snapshot` frames byte-identical before and after (frames.sh), and again with a populated scratch store and a fixture carrying `node_live`; checks.sh and run-tests.sh.
<!-- SECTION:PLAN:END -->

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
Implemented in four commits: row models (8c1cdc1a), overlay and runtime (d380bfee), consistency tests (5a807116), docs (d3e106c5).

Decisions to review:
- The row fields are filled by one post-pass, `tui::rows::live::annotate`, which `build_tree_for_view` runs for every view (the `content::annotate` pattern), rather than by a call inside each builder; builders set `live: None`/empty and nothing else.
- `AgentSessionRow.live: Option<LiveCell>`, `MuxSessionRow.live: LiveCounts` (empty = no evidence), `MixedRow.live: MixedLive` (None / Session / Mux; a pin's row carries its target's value). Search hit and pane rows, PR rows, and group rows carry nothing.
- Mux counts use `GraphSnapshot::live_counts_by_mux(table, now)` (crate-private), which shares the host rule with `live_counts_of_mux`.
- The overlay is on only in the interactive live loop (`LiveOverlay::from_env()`); `--snapshot`, `tui --fixture`, and the scenario TUI leave it off and read the snapshot's own `node_live`, so their frames depend on the snapshot alone.
- On an overlay change, or at most once a second when time alone moved an effective state, the rows on screen are refilled in place (no tree rebuild). A story that sorts, filters, or groups by live state must rebuild instead (noted in the plan README and `tui/app/live.rs`).
- Liveness is checked only at binds (store rewrite or new snapshot), so a killed agent reads `exited/killed` at the next snapshot (<= refresh interval); filed CSP-843.
- Every SetData path (worker/daemon result, `r`, hand-off refresh, static/fixture, snapshot mode) now goes through one `install_snapshot`, which re-binds the overlay before the tree is built.

Verification: frames.sh before/after byte-identical for all nine frames; again identical with a fixture carrying `node_live` for six entities and a populated scratch store (frames-live variant); checks.sh passed.

Validation: run-tests.sh (cargo test --workspace --all-targets --all-features) passed, 5208 passed, 0 failed, 1 ignored; checks.sh (fmt, clippy -D warnings, rustdoc -D warnings, git diff --check) passed; just tuikit-features passed (the framework is unchanged). Filed CSP-843 (re-check bound processes between snapshots).
<!-- SECTION:NOTES:END -->

## Final Summary

<!-- SECTION:FINAL_SUMMARY:BEGIN -->
The TUI now keeps live state on its row models between snapshots, and draws none of it.

- Row models: `AgentSessionRow.live` (`LiveCell`: effective state, detail, confidence, `since_ms`, subagents), `MuxSessionRow.live` (`LiveCounts` over the agents it hosts), and `MixedRow.live` (`MixedLive`, a pin's row carrying its target's value). One pass, `tui::rows::live::annotate`, fills them for every view in `build_tree_for_view` from the live table `TreeInputs` names, read at build time. Mux counts share the snapshot's rule through `GraphSnapshot::live_counts_by_mux`.
- Overlay: `tui::live_overlay::LiveOverlay`, owned by the App and turned on only in the interactive live loop. Each tick it stats the store once. It re-reads and re-binds (`bind_live`, real pid probe) only when the stat shows a rewrite and at least a second has passed since the last read. Rows read its table in place of the snapshot's `node_live`. Every new snapshot (worker or daemon result, `r`, hand-off refresh, fixture, snapshot mode) goes through one `install_snapshot`, which binds the overlay before the rows are built. When the overlay changes, or at most once a second when time alone moved a state, the cells on screen are refilled in place. A missing or corrupt store gives an empty table and reports nothing.
- Verified with overlay unit tests through a fake stat/read seam and a counting pid probe (the throttle, an unchanged file never re-read, missing, corrupt and newer stores, a snapshot swap re-binding). Runtime rig tests write the store through `LiveStore` in a scratch dir and see the rows update within about a second with no discovery, with snapshots from a daemon and without one. Consistency tests check that one entity carries one value in every view, read from the snapshot table and from the overlay. A rendering test finds styled frames identical with and without live state for all six views at three sizes. `tui --snapshot` frames are byte-identical before and after for all nine frames.sh views, and again with a fixture carrying `node_live` and a populated scratch store. Full suite, checks.sh, and `just tuikit-features` passed. Docs: design.md and the plan README.
<!-- SECTION:FINAL_SUMMARY:END -->
