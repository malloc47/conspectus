---
id: CSP-788
title: Make the right pane's hint for an unresolved row say what is true
status: To Do
assignee: []
created_date: '2026-10-30 07:23'
labels:
  - tui-framework
milestone: m-25
dependencies: []
priority: low
ordinal: 874000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Enter on an unresolved-evidence row under the right pane's Other header sets the status line to "explorer: unresolved evidence — `o` opens detail", but `o` on that row only says "explorer: row has no truncated value to open": the property grid (ADR 0187) reports no inspect intent for a link row. Found while extracting the property grid (CSP-754). Either the hint points at where the evidence is shown (the Preview, when the cursor is on the row) or `o` opens the evidence whole, as `o` opens a cut field; the open evidence-inspector work in CSP-318 may decide the second.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 Enter on an unresolved row shows a hint that matches what the keys it names do
- [ ] #2 A test drives Enter and then the named key on an unresolved row and checks the outcome
<!-- AC:END -->

## Definition of Done
<!-- DOD:BEGIN -->
- [ ] #1 `just check` passes, or `git diff --check` for docs-only changes
- [ ] #2 Docs and ADRs are updated when behavior, architecture, or workflow change
<!-- DOD:END -->
