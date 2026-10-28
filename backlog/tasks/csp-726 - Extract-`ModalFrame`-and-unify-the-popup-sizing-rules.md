---
id: CSP-726
title: Extract `ModalFrame` and unify the popup sizing rules
status: To Do
assignee: []
created_date: '2026-10-27 20:26'
labels:
  - tui-framework
  - tuifw-foundation
milestone: m-25
dependencies:
  - CSP-721
  - CSP-723
documentation:
  - docs/plans/tui-framework/inventory/01-design-tokens.md
  - docs/adr/0152-tui-framework-dependency-ownership.md
priority: high
type: enhancement
ordinal: 812000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Modal sizing has 13 distinct rules (the `min(cap, w-4).max(n)` formula appears 10 times) behind 5 `centered_modal_rect` functions. Four popups bypass `themed_popup` with hand-built frames (`row_menu`, `mux_menu`, `worktree_menu`, `clean_up`), eleven identical `KnownSize` impls exist only to satisfy `tui-popup`, and titles take two forms (12 plain, 2 accent-bold). `LinesBody` is dead code (inventory slice 01, section 5).

Scope: Create `tuikit::frame::ModalFrame` with named sizing policies (compact, menu, form, wide), one themed frame and title style, and a body trait that sizes once. Migrate every modal and delete the hand-built frames, the `KnownSize` impls, and `LinesBody`. Remove the `tui-popup` dependency (ADR 0152).

TUI changes:
- Consistency wins: one frame, one title style, and named sizes for every modal; a modal sizes itself once
- Regressions or behavior changes: some modals change width by a few cells when they move to a named policy (for example the mux menu's width)

Tests and checks: The `consistency_tests/layout.rs` checks (nothing escapes the frame at 5 widths by 2 heights, no cut words) pass for every modal. `tui-popup` is gone from `Cargo.lock`.

Blockers: The theme and text extractions.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 `ModalFrame` provides named sizing policies, the themed frame, and one title style, and every modal uses it
- [ ] #2 The 4 hand-built frames, the 11 `KnownSize` impls, `LinesBody`, and the 13 ad hoc sizing rules are removed
- [ ] #3 `tui-popup` is removed from the dependency tree
- [ ] #4 The layout consistency checks pass for every modal at 5 widths and 2 heights
- [ ] #5 Every visible change is listed in the final summary under **Regressions** and **Consistency wins**, and was checked with `conspectus tui --snapshot` at a normal and a narrow width
- [ ] #6 The framework crate names no Conspectus type, string, theme field, or domain word, and the independence guard passes
- [ ] #7 The control has a reference page in the framework docs: purpose, usage, key table, theme tokens used, and the house rules it follows
<!-- AC:END -->

## Definition of Done
<!-- DOD:BEGIN -->
- [ ] #1 `just check` passes, or `git diff --check` for docs-only changes
- [ ] #2 Docs and ADRs are updated when behavior, architecture, or workflow change
<!-- DOD:END -->
