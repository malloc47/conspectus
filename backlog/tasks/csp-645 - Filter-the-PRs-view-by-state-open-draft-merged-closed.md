---
id: CSP-645
title: 'Filter the PRs view by state (open, draft, merged, closed)'
status: Done
assignee:
  - '@claude'
created_date: '2026-10-21 14:53'
updated_date: '2026-10-21 14:53'
labels:
  - tui
  - forge
dependencies: []
priority: medium
type: feature
ordinal: 703000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Seeing only open PRs is the common case in the PRs view, but the only way to separate PRs by state is the state grouping, which also changes the layout. A state filter, like the mux-state filter on the Mux view, keeps the operator's chosen grouping and narrows which PRs show. With enterprise hosts queried, merged and closed PRs make up most of the list.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [x] #1 RowFilter has a PR state set (open, draft, merged, closed); empty means every PR, and open includes drafts while draft matches only drafts
- [x] #2 The PRs view Controls overlay edits it with a multi-select, the status bar shows a state: chip, and it persists and clears with the other filters
- [x] #3 It applies under every grouping, drops groups left empty, and does not switch on the session-based narrowing of PR rows
- [x] #4 conspectus tui and table prs accept --pr-state, and [tui.views.<view>.filters] accepts pr_state with a diagnostic for unknown values
- [x] #5 An ADR records the semantics; tests cover the filter, builder, Controls, chip, CLI, config, and table; docs are updated
<!-- AC:END -->

## Definition of Done
<!-- DOD:BEGIN -->
- [x] #1 `just check` passes, or `git diff --check` for docs-only changes
- [x] #2 Docs and ADRs are updated when behavior, architecture, or workflow change
<!-- DOD:END -->

## Final Summary

<!-- SECTION:FINAL_SUMMARY:BEGIN -->
The PRs view now filters by state: `RowFilter.pr_state` is a set of `open` (drafts included), `draft`, `merged`, and `closed`, empty meaning every PR. It is edited with a multi-select in the PRs Controls overlay, shown as a `state:` status chip, persisted and cleared with the other filters, applied under every grouping (dropping emptied groups) without switching on the session-based narrowing, and available as `--pr-state` on `tui` and `table prs` and as `pr_state` in `[tui.views.<view>.filters]` with a diagnostic for unknown values. ADR 0134 records the semantics. Verified with fmt, clippy `-D warnings`, the full nextest suite (2532 passed; new tests for the matching rules, builder under all groupings, Controls editor, chip, CLI, config, and table) and `tui --snapshot` against this machine's 90-PR world.
<!-- SECTION:FINAL_SUMMARY:END -->
