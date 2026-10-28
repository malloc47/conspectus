---
id: CSP-764
title: Update Conspectus docs and agent instructions for the framework
status: To Do
assignee: []
created_date: '2026-10-27 20:28'
labels:
  - tui-framework
  - tuifw-separation
milestone: m-25
dependencies:
  - CSP-761
documentation:
  - CLAUDE.md
  - docs/adr/README.md
priority: medium
type: docs
ordinal: 850000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
`CLAUDE.md` and `AGENTS.md` carry TUI conventions that now describe framework controls: a new view, menu, or key reuses the existing action, menu, and lifecycle code; `docs/tui-columns.md` is the living reference for the left panel; the consistency tests assert behavior across views; and Pre-Main Invariants name the commands. Operator-facing changes (theme key aliases, removed view values, right-pane keys, new close and confirm behavior) need to reach the operator guide.

Scope: Update `CLAUDE.md` and `AGENTS.md` so the TUI conventions name the framework's controls, key tables, test kit, and house rules, and point to where the consistency tests live now. Update `docs/operations.md` for operator-visible changes and `CHANGELOG.md`. Add the amendments to the ADR index for ADRs 0030, 0031, 0052, 0076, 0085, 0097, and 0123 as their stories landed. Mark the architecture review's findings F4, F5, and F7 as resolved or partly resolved.

TUI changes: none; this documents changes made by other stories.

Tests and checks: `git diff --check` and the doc-sync tests pass. A reader following `CLAUDE.md` to add a menu is sent to the framework's `ActionMenu` page.

Blockers: Complete documentation.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 `CLAUDE.md` and `AGENTS.md` describe the framework-based TUI conventions and the workspace commands
- [ ] #2 `docs/operations.md` and `CHANGELOG.md` list every operator-visible change from the milestone
- [ ] #3 The ADR index records the amendments the milestone made to earlier ADRs
- [ ] #4 `docs/tui-architecture-review.md` marks the findings the milestone resolved
<!-- AC:END -->

## Definition of Done
<!-- DOD:BEGIN -->
- [ ] #1 `just check` passes, or `git diff --check` for docs-only changes
- [ ] #2 Docs and ADRs are updated when behavior, architecture, or workflow change
<!-- DOD:END -->
