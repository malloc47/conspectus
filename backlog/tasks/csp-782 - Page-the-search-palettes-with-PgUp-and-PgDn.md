---
id: CSP-782
title: Page the search palettes with PgUp and PgDn
status: To Do
assignee: []
created_date: '2026-10-29 14:03'
updated_date: '2026-10-29 14:03'
labels:
  - tui-framework
milestone: m-25
dependencies:
  - CSP-743
priority: low
type: enhancement
ordinal: 868000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Quick select and content search are palettes (ADR 0175), and the palette pages by the rows shown with `PgUp`/`PgDn` (ADR 0174, house rule 12), but `SEARCH_KEYS` and `SEARCH_CONTENT_KEYS` in `src/tui/keybindings/overlay.rs` bind neither, so in the search the two keys do nothing while the picker pages. CSP-743 kept the search's key tables as they were, since that story changed no search behavior.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 In quick select and content search, PgUp and PgDn move the cursor by the rows shown and stop at the ends, as in the picker
- [ ] #2 The keys are aliases: the search footers are unchanged
- [ ] #3 A test drives both keys in each mode, and the key-table checks pass
- [ ] #4 The operator guide's quick select and content search sections name the keys
<!-- AC:END -->

## Definition of Done
<!-- DOD:BEGIN -->
- [ ] #1 `just check` passes, or `git diff --check` for docs-only changes
- [ ] #2 Docs and ADRs are updated when behavior, architecture, or workflow change
<!-- DOD:END -->
