---
id: CSP-841
title: Count every pane-only agent on its mux
status: Done
assignee:
  - '@claude'
created_date: '2026-11-12 06:57'
updated_date: '2026-11-12 14:26'
labels:
  - live-state
  - ls-bind
milestone: m-26
dependencies:
  - CSP-812
priority: low
ordinal: 926000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
`node_live` holds one live binding per node (ADR 0205 decision 2). An entry whose agent has no session node yet binds to the tmux mux it runs in, so when two such agents run in panes of one mux (two fresh Pi or Codex sessions before their first transcript) the mux keeps only the entry the store wrote last, and `live_counts_of_mux` counts one agent instead of two until their sessions appear. Once each agent has a session node, both count. The fix needs a place for several pane-only observations per mux, which is a model change (another side table keyed by mux and pane, or a pane sub-key) and touches the snapshot format, the JSON shape, and the binding in `discovery::live_state`.

Related edge seen while binding: the tmux server of an entry is taken from the last component of its socket path, so a `-S` socket whose file name equals a `-L` server's name, or a default server under another `TMUX_TMPDIR`, would match that server's mux of the same session name.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [x] #1 Two pane-only entries in two panes of one mux both count in live_counts_of_mux
- [x] #2 A pane-only entry still moves to its session once the session appears
- [x] #3 Graph JSON and graph.bin carry every pane-only entry, with the format change documented
<!-- AC:END -->

## Definition of Done
<!-- DOD:BEGIN -->
- [x] #1 `just check` passes, or `git diff --check` for docs-only changes
- [x] #2 Docs and ADRs are updated when behavior, architecture, or workflow change
<!-- DOD:END -->

## Implementation Plan

<!-- SECTION:PLAN:BEGIN -->
1. Model: a second sparse side table `GraphSnapshot::pane_live: BTreeMap<LivePane, LiveBinding>`, keyed by tmux mux and pane id (`LivePane { mux, pane }`), for agents seen in a pane that no session matched; `node_live` keeps session entries only. JSON: a `pane_live` list beside `node_live` (each entry the mux `node_id`, `pane`, the observation, `liveness`), omitted when empty; rkyv carries it; FORMAT_VERSION 11 -> 12.
2. Accessors: `live_counts_of_mux` counts hosted sessions and every pane entry of the mux; `live_panes_of(mux)` lists them; `live_state_of` reads sessions (a mux has no state of its own); `live_counts_by_mux` and a borrowed `LiveTables` view for the TUI's overlay table. Eviction drops pane entries with their mux; the fragment conversion and warm merge name the new table.
3. Binding (`discovery::live_state`): rule 2 binds to (mux, pane); when several entries share one pane slot the store's last write wins by the existing rank; `bind_live` returns both tables; `apply_live_state` sets both.
4. Readers: `node show` for a mux lists each pane agent; TUI overlay, rows, recheck, and age check read both tables.
5. Tests: two pane-only entries in two panes both count; one pane keeps the last write; a pane entry moves to its session; JSON and rkyv round trips; eviction; pinned format version; cold rebuild of an older graph.bin. Docs: ADR 0205 text, design.md, library-api.md, operations.md JSON note, plan README.
6. File the socket-name edge (a `-S` socket named like a `-L` server, or another TMUX_TMPDIR) as a follow-up.
<!-- SECTION:PLAN:END -->

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
Representation: a second sparse table `GraphSnapshot::pane_live: BTreeMap<LivePane, LiveBinding>` (LivePane = mux id + optional pane id) instead of a pane sub-key in `node_live`'s key. A sub-key would make every session reader and about 40 test sites build a compound key and would let a session key carry a pane; the second table leaves session readers untouched and keeps `node_live` one entry per node. Cost: the TUI passes a borrowed `LiveTables` (both maps) where it passed one map, and `bind_live` returns `BoundLive`.
Entries with no pane id (or an empty one) share one slot per mux, so they still collapse as before; writers record `TMUX_PANE`, so this should be rare.
`live_state_of(mux)` now returns None (it read the mux's single pane entry before); nothing outside tests used it for muxes. A mux key put into `node_live` by hand still counts toward that mux.
Follow-up filed: CSP-853 (socket-name matching edge from the description).
Validation: narrow runs of discovery, model, output, tui live, snapshot, graph_snapshots, live_state_output, testing_replay, hook_write; `checks.sh` exit 0; `run-tests.sh` exit 0 (5382 passed, 0 failed, 1 ignored); frames.sh nine frames byte-identical to the base.
<!-- SECTION:NOTES:END -->

## Final Summary

<!-- SECTION:FINAL_SUMMARY:BEGIN -->
Two agents with no session yet in two panes of one tmux mux now both count. Pane-only entries moved from `node_live` (one per node, keyed by the mux) to a new sparse table `GraphSnapshot::pane_live`, keyed by `LivePane` (mux id and pane id), one entry per pane, with the existing last-write rank per pane. `node_live` keeps session entries only. `live_counts_of_mux` counts hosted sessions plus every pane entry; `live_panes_of` lists them; `node show` prints each with its pane. `bind_live` returns `BoundLive` (both tables); discovery, warm merge, eviction, the TUI overlay (including the process re-check and age check), and rows read both through `LiveTables`. A pane entry still moves to its session once the session appears (tests at unit and replay level).

Format: graph JSON gains a `pane_live` list (mux `node_id`, `pane`, observation fields, `liveness`), omitted when empty; `graph.bin` FORMAT_VERSION 11 -> 12 (the relative previous-version refusal and cold-rebuild tests pass; a new graph.bin round trip carries three pane entries). Documented in ADR 0205, docs/design.md, docs/library-api.md, docs/operations.md, the plan, and the unreleased CHANGELOG entry.

Goldens changed: graph_snapshots live_state JSON and live_state_output mux node show (the fixture's mux entry moved to pane `%2` and a second pane agent `%3` was added). TUI frames byte-identical. Verified with `checks.sh` and the full `run-tests.sh` (5382 passed, 0 failed). Follow-up: CSP-853.
<!-- SECTION:FINAL_SUMMARY:END -->
