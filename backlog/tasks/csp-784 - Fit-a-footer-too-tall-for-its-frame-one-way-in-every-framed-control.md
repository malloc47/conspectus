---
id: CSP-784
title: Fit a footer too tall for its frame one way in every framed control
status: To Do
assignee: []
created_date: '2026-10-29 18:15'
labels:
  - tui-framework
milestone: m-25
dependencies: []
priority: low
ordinal: 870000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
When a modal's footer has more rows than its frame leaves for it, the framed controls disagree about which rows stay. The pager (and so the help page, the value modal, and every framed reader) keeps the footer's last rows, the keys that leave, and scrolls its first rows with the body (ADR 0178). Forms keep the footer's first rows and drop the rest (FormBody::bands), dialogs and checklists keep the bottom rows, and palettes cap the footer at four rows. On a short terminal the same footer therefore loses different keys in different modals, against house rule 8 of ADR 0151. CSP-746 settled the rule for pagers and found the others while deciding it; CSP-781 settles the footer's look, not which rows stay.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 Every framed control (form, dialog, checklist, settings panel, palette, action menu, pager) keeps the same footer rows when the footer does not fit, by one shared framework rule
- [ ] #2 The rule is recorded in an ADR and in each affected control's reference page
- [ ] #3 Tests cover a too-tall footer for each framed control at a short size
- [ ] #4 Every visible change is listed in the final summary under Regressions and Consistency wins, checked with conspectus tui --snapshot at a normal and a narrow width
<!-- AC:END -->

## Definition of Done
<!-- DOD:BEGIN -->
- [ ] #1 `just check` passes, or `git diff --check` for docs-only changes
- [ ] #2 Docs and ADRs are updated when behavior, architecture, or workflow change
<!-- DOD:END -->
