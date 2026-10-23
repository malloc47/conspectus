---
id: CSP-695
title: >-
  Fix mux exit refresh, launch with skip permissions, and the attached glyph
  width
status: To Do
assignee: []
created_date: '2026-10-23 22:07'
labels:
  - tui
milestone: m-23
dependencies: []
priority: high
type: bug
ordinal: 768000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Third round of hands-on testing: a mux that was exited still shows as not fully stopped until a manual refresh even after the earlier cache fix, launching a harness in a new mux with skip permissions fails with an unrecognized argument, the launch form defaults to a worktree, its command row looks editable, and the attached glyph is wider than one cell so a selection highlight cuts it in half.
<!-- SECTION:DESCRIPTION:END -->

## Definition of Done
<!-- DOD:BEGIN -->
- [ ] #1 `just check` passes, or `git diff --check` for docs-only changes
- [ ] #2 Docs and ADRs are updated when behavior, architecture, or workflow change
<!-- DOD:END -->
