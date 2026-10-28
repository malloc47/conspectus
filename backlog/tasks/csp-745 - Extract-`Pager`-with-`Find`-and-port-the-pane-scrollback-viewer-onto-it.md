---
id: CSP-745
title: Extract `Pager` with `Find` and port the pane scrollback viewer onto it
status: To Do
assignee: []
created_date: '2026-10-27 20:27'
labels:
  - tui-framework
  - tuifw-reading
milestone: m-25
dependencies:
  - CSP-733
  - CSP-730
  - CSP-724
  - CSP-723
  - CSP-729
  - CSP-725
documentation:
  - docs/plans/tui-framework/inventory/05-search-help-viewers.md
  - docs/adr/0052-native-session-viewer.md
priority: high
type: enhancement
ordinal: 831000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Seven scrolling surfaces are one shape: the transcript viewer, the pane scrollback viewer, the keymap help, the viewer help, the pane help, the value modal, and the Messages detail pane. The transcript and pane viewers share 14 of their 15 to 18 messages, 20 key rows, and 12 help sentences, yet behave differently in find unit, scroll-on-match, end-sticky, and wording. The five pager key tables repeat about 55 movement rows. Page size is a fixed 8 lines in three surfaces and viewport-sized in the others (inventory slice 05).

Scope: Create `Pager<C: PagerContent>` with modes Screen (header, rule, body, rule, footer), Modal, and Embedded; the `PagerContent` trait (rows for a width, match units, header, extra chips, extra key rows with descriptions, apply); `Find` on `LineEdit` with `n` and `N`, wrap notes, and a current-match marker; Esc stepping (help, then find, then leave); the sticky-end rule; viewport paging and half-page keys; clamp-on-draw; a generated `?` page from `HelpPage`; and a copy hook. Port the pane scrollback viewer first, as a roughly 60-line content adapter. Apply house rule 18: Esc steps back, `q` leaves, a confirmed find scrolls to the match, a `[ n/m ]` chip in Screen mode.

TUI changes:
- Consistency wins: the pane viewer gets the transcript viewer's find, paging, and help behavior; one position indicator rule
- Regressions or behavior changes: the pane viewer's find unit and scroll-on-confirm change to the pager's; long lines in the pane viewer wrap or scroll sideways instead of being clipped (the story picks one and says so)

Tests and checks: Port the pane viewer's tests. Property tests for the scroll window and sticky-end. `--snapshot` of the pane viewer at 80x24 and 40x10, with and without find.

Blockers: `LineEdit`, the help extraction, scroll, text, the overlay contract, and chips.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 `Pager<C>` provides Screen, Modal, and Embedded modes, `Find`, Esc stepping, sticky-end, viewport and half-page paging, clamp-on-draw, and a generated help page
- [ ] #2 The pane scrollback viewer is a `PagerContent` adapter of about 60 lines and its duplicate help page and key table are removed
- [ ] #3 The pane viewer's behavior for each documented difference from the transcript viewer is decided and listed in the final summary
- [ ] #4 A `[ n/m ]` position chip appears in Screen mode
- [ ] #5 Every visible change is listed in the final summary under **Regressions** and **Consistency wins**, and was checked with `conspectus tui --snapshot` at a normal and a narrow width
- [ ] #6 The framework crate names no Conspectus type, string, theme field, or domain word, and the independence guard passes
- [ ] #7 The control has a reference page in the framework docs: purpose, usage, key table, theme tokens used, and the house rules it follows
<!-- AC:END -->

## Definition of Done
<!-- DOD:BEGIN -->
- [ ] #1 `just check` passes, or `git diff --check` for docs-only changes
- [ ] #2 Docs and ADRs are updated when behavior, architecture, or workflow change
<!-- DOD:END -->
