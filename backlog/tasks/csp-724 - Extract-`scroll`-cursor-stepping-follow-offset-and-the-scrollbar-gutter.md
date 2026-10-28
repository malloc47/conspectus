---
id: CSP-724
title: 'Extract `scroll`: cursor stepping, follow offset, and the scrollbar gutter'
status: To Do
assignee: []
created_date: '2026-10-27 20:26'
labels:
  - tui-framework
  - tuifw-foundation
milestone: m-25
dependencies:
  - CSP-718
  - CSP-720
  - CSP-712
documentation:
  - docs/plans/tui-framework/inventory/01-design-tokens.md
  - docs/plans/tui-framework/inventory/07-chrome-and-explorer.md
priority: medium
type: enhancement
ordinal: 810000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Cursor stepping lives in `cursor.rs` (wrap in lists, clamp in the tree, explorer, and menus), scroll-to-cursor is copied five times, and the scrollbar has five call sites, one a copy in `widgets/help.rs`. Scroll offsets are written back by the renderer (`Cell`s on `App`) and by messages dispatched during `draw`. Wrapped-row counting has four implementations (inventory slices 01 and 07).

Scope: Create `tuikit::scroll` with `wrap_step`, `clamp_step`, a follow-cursor offset function, a scroll window type that clamps on use, the fade-on-fit scrollbar gutter, and one wrapped-row counter. Migrate the five scroll-to-cursor copies, the five scrollbar sites, and the four wrapped-row counters. The renderer reports the measured viewport to the owner through a return value, not a dispatched message.

TUI changes:
- Consistency wins: offsets are always clamped, so no blank tail; one scrollbar look everywhere
- Regressions or behavior changes: none expected

Tests and checks: Property tests: the cursor is always inside the window after any step sequence; the window is always inside the content. Existing scroll tests in `ui_tests.rs` pass.

Blockers: The scroll and paging defect task, so the fixes land before the code moves.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 `tuikit::scroll` provides stepping, a follow offset, a clamping scroll window, the scrollbar gutter, and one wrapped-row counter
- [ ] #2 The five scroll-to-cursor copies, the five scrollbar sites, and the four wrapped-row counters use it
- [ ] #3 No renderer dispatches a message during `draw` to report a viewport
- [ ] #4 Property tests cover the cursor and window invariants
- [ ] #5 Every visible change is listed in the final summary under **Regressions** and **Consistency wins**, and was checked with `conspectus tui --snapshot` at a normal and a narrow width
- [ ] #6 The framework crate names no Conspectus type, string, theme field, or domain word, and the independence guard passes
- [ ] #7 The control has a reference page in the framework docs: purpose, usage, key table, theme tokens used, and the house rules it follows
<!-- AC:END -->

## Definition of Done
<!-- DOD:BEGIN -->
- [ ] #1 `just check` passes, or `git diff --check` for docs-only changes
- [ ] #2 Docs and ADRs are updated when behavior, architecture, or workflow change
<!-- DOD:END -->
