---
id: CSP-826
title: 'TUI: order, group, and filter by live state'
status: Done
assignee:
  - '@claude'
created_date: '2026-11-12 01:48'
updated_date: '2026-11-13 02:54'
labels:
  - live-state
  - ls-tui
milestone: m-26
dependencies:
  - CSP-823
documentation:
  - docs/plans/live-state/tui-options.md
priority: low
type: enhancement
ordinal: 912000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Operators of other tools sort or group by "needs me first". Rows that re-sort under the cursor are disorienting, and Conspectus keeps rows still on fold, so whether this is a sort control, a "state" grouping in the cycle, a filter, or a dedicated attention view needs a decision.

Scope: Per the operator's decision: sort and grouping options, a `--state` filter in the CLI, a Controls row, filter chips with counts, and a stability rule so the cursor and the operator's place do not jump.

TUI changes: Controls, sort, and grouping gain options as decided.

Tests and checks: Sort and filter tests; cursor stability under live changes; snapshot frames.

Blockers: Operator decision 4 and the cell story.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [x] #1 Sessions-view Controls has a 'Float waiting sessions to top' checkbox, off by default; when on, sessions whose mark is ! sort above the rest of their group, of the flat list, and of a parent's forks, keeping the existing order within each part and taking precedence over the muxed-sessions float
- [x] #2 Mux-view Controls has a 'Float muxes with a waiting agent to top' checkbox, off by default; when on, running muxes with a waiting agent (their mark is !) sort above the other running muxes, ahead of the attached-muxes float; stopped muxes stay below
- [x] #3 Both float checkboxes persist per view in tui-state.json like their siblings and Clear all filters (F) resets them
- [x] #4 Controls has an 'agent state' row beside 'mux state' that steps any, waiting, ready, working, error, any; Sessions, Mux, PRs and Search keep only sessions whose mark matches (a running mux stays when an agent attached to it or in one of its panes matches, its listed agents narrowed to the matching ones); Pinned and History ignore it like the other filters; it persists per view
- [x] #5 With the filter set the status bar shows agent:<value> among the filter chips, the header reads N/M sessions, and the empty state names the chip
- [x] #6 conspectus table <rows> --state (also tui and search) takes waiting, ready, working, error, unknown, none, comma-separated or repeatable, read at the current clock; a mux row matches when an agent it hosts matches; an invalid value errors listing the values; [tui.views.<name>.filters] state sets it from config with a diagnostic for an invalid value
- [x] #7 Stability: a store change that moves a session's state refills its mark in place; with the float on and the filter set its row, the order, the membership and the cursor do not move until the tree is next built (a control or filter change, r, a new snapshot), which a test covers
- [x] #8 No state grouping joins the Ctrl-G cycle and no Attention view is added
- [x] #9 docs/tui-columns.md, docs/operations.md, CHANGELOG, design.md and the plan describe the controls, the filter and the stability rule; frames at 170, 100, 70 and 50 with Controls open are reviewed and the consistency suite covers the new rows
<!-- AC:END -->

## Definition of Done
<!-- DOD:BEGIN -->
- [x] #1 `just check` passes, or `git diff --check` for docs-only changes
- [x] #2 Docs and ADRs are updated when behavior, architecture, or workflow change
<!-- DOD:END -->

## Implementation Plan

<!-- SECTION:PLAN:BEGIN -->
1. ADR 0215: the agent-state filter (values, mapping to the marks, mux rule, CLI/config/Controls/chip), the float-waiting-first control per view, and the stability rule (order and membership read when the tree is built, never on an overlay refill). Index it and list it among the host records the framework log leaves out.
2. Model: `LiveStateKey`/`LiveStateFilter` in `src/filter.rs` (one mapping shared with `LiveMark`), `RowFilter.live_state`, `float_waiting_top`, and `live` (each session's and mux's state, injected when a tree is built, never saved); `SessionMatchInputs.live_state`.
3. TUI builders: Sessions, Mux (attached agents and pane-only agents), PRs, Search filter through it; Sessions and Mux float waiting first; `build_tree_for_view` injects the states read at the same moment as the rows' values. Overlay refills stay in place.
4. Controls: an `agent state` cycle row beside `mux state`; the two float checkboxes; status-bar chip `agent:`; Clear all resets them.
5. CLI: `--state` on `tui`, `table`, `search` (comma-separated/repeatable, like `--mux-state`), effective at the current clock; `[tui.views.<name>.filters] state`.
6. Tests: filter unit tests, builder tests, Controls tests, CLI tests, the stability test, consistency suite (one mapping between mark and filter, chip, every view).
7. Docs: tui-columns.md, operations.md, CHANGELOG, design.md, plan README; frames before/after at 170/100/70/50 with Controls open.

8. Revised while building: a mux passes under a lone live-state filter through every agent its mark counts (preferred-mux sessions, ambiguous ones included, and pane-only agents), not only attached and pane agents, after the showcase's ambiguous mux, drawn !, vanished under agent:waiting; the Mux checkbox label became 'Float waiting muxes to top' to fit 50 columns and match 'Float attached muxes to top'.
<!-- SECTION:PLAN:END -->

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
Operator decision (2026-11-12), see docs/plans/live-state/README.md section 7.1 item 4: a float-waiting-first control (off by default, the same mechanism as float-sessions-with-a-mux), a State filter row in Controls, and --state on conspectus table. Rows never re-sort under the cursor between operator actions. No state grouping and no Attention view in this story. Note from CSP-814: sorting or filtering by live state means the tree must be rebuilt when the overlay changes, instead of refilling cells in place.

Validation: checks.sh (fmt, clippy -D warnings, rustdoc -D warnings, git diff --check) exit 0; run-tests.sh exit 0, 5517 passed, 0 failed (slice fd2891b9 alone: 5504 passed). Frames: wframes.sh (showcase, widths 170/100/70/50, height 40) before/after: sessions, flat sessions, mux, prs, pinned, history byte-identical; only the Controls frames changed. Extra frames with the float and the filter applied (Sessions graph and flat, Mux) reviewed.
Follow-ups filed: CSP-861 (Mux view N/M under a filter counts only linked sessions), CSP-862 (table mux matches the content filter by state scope, a latent bug found here).
<!-- SECTION:NOTES:END -->

## Final Summary

<!-- SECTION:FINAL_SUMMARY:BEGIN -->
Landed the operator's decision 4 (ADR 0215). Controls gains an `agent state` row after `mux state` in every view (Space steps any, waiting, ready, working, error; a CLI or config set reads as its values and steps to any), `Float waiting sessions to top` (Sessions) and `Float waiting muxes to top` (Mux), off by default and kept per view in tui-state.json. The filter and the float read every agent's state once per tree build (`RowFilter.live`), from the tables and moment the rows' marks use; the overlay refill still changes only marks, counts, and the detail pane, so rows keep still between operator actions. One mapping (`filter::LiveStateKey`) decides the marks and the filter. A mux passes through an attached agent or, under a lone live-state filter, any agent its mark counts (ambiguous preferred-mux sessions and pane agents included). `--state` on table, tui, search (and dev scenario tui), and `state` in `[tui.views.<name>.filters]`. Chip `agent:<value>`, header N/M, empty state named. No state grouping, no Attention view.

Verified: checks.sh exit 0; run-tests.sh exit 0, 5517 passed; consistency `live_order` (filter keeps exactly the rows with its mark in Sessions, Mux, PRs, Search; float lifts `!` rows; store change moves no row, order, membership, or cursor until a rebuild; folds build nothing; chip and N/M; Pinned and History unchanged); builder, Controls, CLI, config, tui-state, and `table` integration tests. Frames at 170/100/70/50: default views byte-identical; Controls frames changed.

**Regressions**
- Controls is two rows longer in Sessions and Mux and one row longer elsewhere, so on short screens it scrolls sooner (at 40 rows, Sort's `Recency` scrolls out of view).
- The help page's "Waiting on you" legend reads longer.
- A floated or filtered list can show a row whose mark no longer matches (an answered wait among the floated `!` rows, a `›` under `agent:waiting`) until the next build, by design.
- Mux view under `agent:waiting`: the `ambiguous` mux is listed with `0/11 sessions` in the header, since N counts linked sessions (CSP-861).

**Consistency wins**
- The marks and the filter share one mapping, so `agent:waiting` lists exactly the `!` rows; a mux drawn `!` stays under it (also in `table mux`).
- `--state` sits wherever `--mux-state` does (table, tui, search, scenario tui, config filters); the new rows and labels mirror their siblings (`Float waiting muxes to top` beside `Float attached muxes to top`).
- The stability rule is written down (docs/tui-columns.md) and tested.

Follow-ups: CSP-861, CSP-862.
<!-- SECTION:FINAL_SUMMARY:END -->
