---
id: CSP-854
title: 'TUI: show running subagents in live state'
status: To Do
assignee: []
created_date: '2026-11-12 15:25'
updated_date: '2026-11-12 20:06'
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
- [ ] #1 Pending the operator's decisions
<!-- AC:END -->

## Definition of Done
<!-- DOD:BEGIN -->
- [ ] #1 `just check` passes, or `git diff --check` for docs-only changes
- [ ] #2 Docs and ADRs are updated when behavior, architecture, or workflow change
<!-- DOD:END -->

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
Operator decision (2026-11-12), see docs/plans/live-state/README.md section 7.1 item 8: a dim '+N' chip after the row label while any subagent is active, and the detail pane lists them; no nested rows. A subagent that is waiting raises '!' on the root session (already in the model).
<!-- SECTION:NOTES:END -->
