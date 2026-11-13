---
id: CSP-854
title: 'TUI: show running subagents in live state'
status: Done
assignee:
  - '@claude'
created_date: '2026-11-12 15:25'
updated_date: '2026-11-12 23:29'
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
ordinal: 939000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Live state counts the subagents a session runs (`subagents`, shown as `+N` in `conspectus table`) and raises a subagent's wait to its root session (`via_subagent`). How the TUI shows them is open: a dim `+N` chip after the row's mark, nested rows per subagent, or only a line in the detail pane (`docs/plans/live-state/tui-options.md` section 6 recommends the chip, no nested rows). Nested rows would multiply rows during a parallel burst; CSP-297 (nesting subagent sessions) is related but covers discovered subagent sessions, not the live count.

Scope: Per the operator's decision 8 in the plan's section 7: the chip or rows, the detail-pane line, the glossary entry, and the `docs/tui-columns.md` rows and frames if a column or glyph changes. Reuses the row models' `live` fields.

TUI changes: Session rows (and the explorer) show running subagents as decided.

Tests and checks: `conspectus tui --snapshot` with sessions that run subagents and one whose subagent waits; `tests/tui_columns_doc.rs`; the consistency suites.

Blockers: Operator decision 8 and the cell story (CSP-823).
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [x] #1 A session row (in Sessions, and nested under a mux or a PR, and in Search) whose agent reports one or more active subagents shows a dim `+N` chip right after its label cell, before the `was in` note, the project, the hit count, and the preview; with no label it follows the link cell
- [x] #2 N reads exactly up to 9 and `+9+` from ten on, so the chip is at most three cells; the chip shows nothing once the session reads exited or unknown, by the same rule `conspectus table` uses for its `+N`, and nothing without live state
- [x] #3 Mux rows (running and stopped) and Pinned and History rows show no subagent chip
- [x] #4 On a narrow row the chip gives way after the preview, the `was in` note, and the project, and before the hit count and the label, which keeps its width while the chip can leave
- [x] #5 One session shows one chip in every view that lists it; a store change refills it in place
- [x] #6 The showcase has a session running subagents and one whose subagent waits; docs/tui-columns.md (session row, chips, narrow rows, frames), the help legend, docs/operations.md, CHANGELOG, and an ADR describe the chip; the consistency suite covers the views, the width rules, and the text rules
<!-- AC:END -->

## Definition of Done
<!-- DOD:BEGIN -->
- [x] #1 `just check` passes, or `git diff --check` for docs-only changes
- [x] #2 Docs and ADRs are updated when behavior, architecture, or workflow change
<!-- DOD:END -->

## Implementation Plan

<!-- SECTION:PLAN:BEGIN -->
1. Model: LiveState::shows_subagents (not exited or unknown), used by the CLI's `+N` and a new LiveCell::running_subagents, so the TUI and the table agree.
2. badge.rs subagents_chip: an inert StatusChip `+N`, capped at `+9+`.
3. Session schema: a hug Subagents column after the label (gives way after the project, before the hit count and the label); session_cells pushes it when running_subagents > 0. Mux schema untouched.
4. Tests: chip label and cap; LiveCell rule; row layout (position, no label, narrow give-way order, width bound); consistency: one chip per session in every view, none on mux/Pinned/History rows, exited/unknown none, in-place refill; adjust the "nothing else" live test to separate the chip.
5. Showcase: the pinned session's wait is a subagent's (via_subagent, 1 subagent), ambiguous candidate B runs 3 subagents; regenerate the fixture's live tables and the frames.
6. Docs: ADR 0212, tui-columns.md, help legend `+N`, operations.md, CHANGELOG, design.md, plan README.
<!-- SECTION:PLAN:END -->

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
Operator decision (2026-11-12), see docs/plans/live-state/README.md section 7.1 item 8: a dim '+N' chip after the row label while any subagent is active, and the detail pane lists them; no nested rows. A subagent that is waiting raises '!' on the root session (already in the model).

Chose: the cap `+9+` (exact through 9, at most three cells); the chip hides for exited and unknown states through a new LiveState::shows_subagents that `conspectus table` now reads too (output unchanged); session rows only (mux, Pinned, and History rows show none); give-way order preview, `was in`, project, chip, hit count, label (the hit count outlasts the chip because it is why a row is in a search's results). The detail pane's list of subagents is CSP-829's (decision 8's other half).
Validation: run-tests.sh exit 0 (5,455 passed, 0 failed, 1 ignored); checks.sh exit 0; frames.sh baseline vs final and shots.sh at 170/100/70/50.
<!-- SECTION:NOTES:END -->

## Final Summary

<!-- SECTION:FINAL_SUMMARY:BEGIN -->
A session row whose agent runs subagents shows a dim `+N` right after its label (after the link cell when it has none), before the `was in` note, the project, the hit count, and the preview, in Sessions, the agents nested under a mux or a PR, and Search. It reads `+1` to `+9`, then `+9+`, so it is at most three cells; it shows while the agent reports one or more and its state is neither exited nor unknown, the rule `conspectus table` now shares (`LiveState::shows_subagents`). Mux, Pinned, and History rows show none. On a narrow row it gives way after the preview, the `was in` note, and the project, before the hit count and the label. The help legend gains a `+1` row. The detail pane's list of subagents stays with CSP-829. ADR 0212.

Commits: 25775a64 (ADR 0212), 23683a3e (model rule shared with the CLI), d2842cdc (feature and tests), bd6b4c6b (showcase), 3f928784 (docs).

**Regressions**
- A row with the chip loses its preview four cells sooner: at 170 columns the pinned `project` session's preview no longer shows in Sessions; in PRs both chipped rows' previews are cut sooner; in flat grouping the pinned session's project reads `proj…`.
- Where the label fits but the chip does not (the 150-column split's cursor row), the chip leaves without a mark, by the give-way rule.
- The help page's Agent state section has one more row.

**Consistency wins**
- The TUI and `conspectus table` hide a stale or exited session's subagents by one rule.
- One session shows one chip in every view, in the look of the other notes after a label.
- Mux rows are unchanged, and a subagent's wait already reads `!` on its root (the showcase now shows both).

Visible changes: candidate B `+3` and the pinned session `+1` in Sessions (graph, repo, and flat groupings), PRs, and Search; none in the Mux view, Pinned, or History.

Verified with run-tests.sh (5,455 passed, 0 failed), checks.sh, the reference frames, frames.sh before/after, and snapshots at 170/100/70/50.
<!-- SECTION:FINAL_SUMMARY:END -->
