---
id: CSP-870
title: Widen the pane-title source to non-active panes and wrapped Codex launches
status: Done
assignee:
  - '@claude'
created_date: '2026-11-13 04:56'
updated_date: '2026-11-13 21:16'
labels:
  - live-state
  - ls-policy
milestone: m-26
dependencies:
  - CSP-831
priority: low
ordinal: 950000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
The pane-title source (ADR 0217) reads only each tmux session's active pane, from the existing `list-sessions` call, and only panes whose program tmux reports as `codex`. A Codex in another pane of a split window or in a window that is not current gives it nothing, and a Codex started from an npm install shows as `node`. Covering them needs either a second tmux call per cycle (`list-panes -a`) or a way to recognize a wrapped Codex without guessing, so it waits until it is worth the cost.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [x] #1 ADR 0221 decides to read every pane's title from the pane listing ADR 0220 already runs (no further tmux call per cycle) and amends ADR 0217
- [x] #2 A Codex in a non-active pane shows its title's state on its own pane, with tests over a multi-pane listing (probe, binding, discovery)
- [x] #3 Codex launched through a wrapper (a pane reporting node) is recognized only where a live Codex hook entry names its pane, the record the hook sidecar would attribute there; with no hook entry its title binds nothing
<!-- AC:END -->

## Definition of Done
<!-- DOD:BEGIN -->
- [x] #1 `just check` passes, or `git diff --check` for docs-only changes
- [x] #2 Docs and ADRs are updated when behavior, architecture, or workflow change
<!-- DOD:END -->

## Implementation Plan

<!-- SECTION:PLAN:BEGIN -->
Decision: widen, at no further tmux call. The pane listing ADR 0220 added (one `list-panes -a` per server with sessions, every cycle) carries `#{pane_title}` last, so every pane's title is read there and classified at the split by its own pane's program; the session listing stops asking for the active pane's title.
1. tmux probe: pane rows carry the title mark (dropped text); the session listing loses `#{pane_title}`; `MuxProbe.titles` is keyed by pane id (runs carried per pane); CONSPECTUS_DISABLE_PANE_TITLES leaves the title out of the pane listing. Snapshot format 17.
2. Wrapped launches: a harness adapter may name launcher programs its titles show under (Codex: `node`). A pane running a launcher has its title classified by that harness, and the mark counts only where that harness's live hook entry names the pane (the record the hook sidecar would attribute there); with no hook entry it binds nothing.
3. Binding: each titled pane binds on its own pane of its mux, with the harness its own program names (or launches); hook status counts title-sourced agents by each pane's own program.
4. Tests: multi-pane listing with Codex in a non-active pane (probe, binding, discovery), wrapped Codex with and without a hook entry, the switch, sentinel over every pane's title.
5. Docs: ADR 0221 amending 0217 (decisions 1, 2, 6, 7, 9), index, operations, design, library API, CHANGELOG.
<!-- SECTION:PLAN:END -->

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
Decision (ADR 0221): widen. The cost question resolved itself once CSP-852 put `list-panes -a` in every probe: the title rides that call as its last field and the session listing drops `#{pane_title}`, so no call is added. Wrapped launches: adapters may name launcher programs (`pane_title_launchers`; Codex lists `node`); a launcher pane's mark binds only through a live hook entry of that harness naming the pane (rule 1 off). A no-pane hook entry is matched only to the active pane's title now that other panes have titles (ProbedPane.active). Visible: frames.sh's showcase runs with tmux disabled, so its frames cannot differ; no rendering code changed. Validation: title_tests rewritten for the pane listing (parser incl. launcher and claude panes, tabs, format switch, runs per pane, per-server runs, sentinel in every pane's title through binding, JSON, Debug, graph.bin), live_title_tests (non-active panes, hook wait in a non-active pane, no-pane entry, launcher with/without Codex hook entry, dead agent), discovery tests (split-pane Codex; npm Codex cleared by two probes only with its hook entry), hook_health title count by pane program. checks.sh green; full suite 5,669 passed, 0 failed.
<!-- SECTION:NOTES:END -->

## Final Summary

<!-- SECTION:FINAL_SUMMARY:BEGIN -->
Decided to widen (ADR 0221, amending ADR 0217): every pane's title is read from the `list-panes -a` call ADR 0220 already makes each cycle (title as its last field), and the session listing no longer asks for one, so the cost is no extra tmux call. Each title is classified at the split by its own pane's program and only the mark is kept, by pane id (`MuxProbe.titles`), beside each pane's session, program, and active flag (snapshot format 17). A Codex in a split or background pane now binds on its own pane or replaces its own hook entry's slot. Wrapped launches: a harness may list launcher programs (Codex: `node`); a launcher pane's title is read as that harness's and counts only where the harness's live hook entry names the pane, never on its own. A hook entry that names no pane is matched only to the active pane's title. `hook status` counts title-read agents by each pane's own program. Docs: ADR 0221, ADR 0217 note, index, manifest citation, operations, design, library API, CHANGELOG.

Verified by the rewritten probe title tests (incl. a sentinel in every pane's title through binding, JSON, Debug, and graph.bin), new binding tests for non-active and launcher panes, discovery tests over multi-pane fake listings, and a hook-status count test; fmt, clippy -D warnings, rustdoc -D warnings, git diff --check, and the full suite (5,669 passed, 0 failed) pass. No rendering code changed; the snapshot fixture runs without tmux, so its frames are unaffected.
<!-- SECTION:FINAL_SUMMARY:END -->
