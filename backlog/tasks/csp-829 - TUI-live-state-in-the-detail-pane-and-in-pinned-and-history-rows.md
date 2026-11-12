---
id: CSP-829
title: 'TUI: live state in the detail pane and in pinned and history rows'
status: To Do
assignee: []
created_date: '2026-11-12 01:48'
updated_date: '2026-11-12 20:05'
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
ordinal: 915000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
The explorer should say what state a node is in, since when, how old the evidence is, where it came from, and whether it is quiet or stale. Pinned and History use words, so a pinned live session also needs its word decided (replace `in mux`, or compose with it).

Scope: Per the operator's decision: Node fields in the explorer, the glossary entry, and the Pinned and History words.

TUI changes: The detail pane and two views gain fields or words.

Tests and checks: Explorer and view snapshots; glossary tests.

Blockers: Operator decisions 7 and 12, and the cell story.
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
Operator decision (2026-11-12), see docs/plans/live-state/README.md section 7.1 items 7 and 12: Node fields State, Since, Source, and Evidence (fresh, quiet, stale) in the explorer, the glossary entry, and the Pinned and History words from CSP-823. Quiet states show dimmed; stale or unknown shows '?'.
<!-- SECTION:NOTES:END -->
