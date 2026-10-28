---
id: CSP-776
title: Dim overlay actions that cannot act now
status: To Do
assignee: []
created_date: '2026-10-28 20:31'
labels:
  - tui-framework
dependencies: []
priority: low
ordinal: 862000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
The key tables now carry availability on each row (`tuikit::keys::with_availability`, ADR 0160), and the help pages dim an unavailable row, but no overlay sets it and most overlay footers are still drawn from the plain `footer` string, which cannot dim a segment. Today a footer advertises keys that do nothing in the moment: Messages shows `y copy` and `J/K scroll detail` on an empty log, and pin edit shows `Ctrl-R follow again` on a field with nothing to follow. This is the gap analysis's tri-state availability (show, dim, hide), Tier 1.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 Messages dims its select, detail, and copy hints while the log is empty, and a key that cannot act explains itself instead of doing nothing silently
- [ ] #2 Pin edit dims Ctrl-R while the focused field has no override
- [ ] #3 Every generated overlay footer is drawn through tuikit::keys::Footer so availability reaches it
- [ ] #4 Visible changes are checked with conspectus tui --snapshot at a normal and a narrow width
<!-- AC:END -->

## Definition of Done
<!-- DOD:BEGIN -->
- [ ] #1 `just check` passes, or `git diff --check` for docs-only changes
- [ ] #2 Docs and ADRs are updated when behavior, architecture, or workflow change
<!-- DOD:END -->
