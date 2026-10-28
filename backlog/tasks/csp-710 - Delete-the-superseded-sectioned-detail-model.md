---
id: CSP-710
title: Delete the superseded sectioned detail model
status: To Do
assignee: []
created_date: '2026-10-27 20:26'
labels:
  - tui-framework
  - tuifw-cleanup
milestone: m-25
dependencies: []
documentation:
  - docs/plans/tui-framework/inventory/07-chrome-and-explorer.md
priority: medium
type: chore
ordinal: 796000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
The right pane's explorer replaced the old sectioned detail model, but `src/tui/detail.rs` (1,409 lines, plus 1,180 lines of tests) and most of `src/tui/ui/detail_header.rs` (376 lines) are still in the tree. Production code reads only `NodeDetail.kind` and `ambiguous_muxes_for_group` from them, and the `e` key on the left pane toggles a flag that has no visible effect. Left in place, the model would be inventoried, extracted, or documented by mistake, and it hides which code the TUI actually runs. The inventory flags it as delete-not-extract (slice 07).

Scope: Retarget the two live reads onto the explorer or rows modules. Delete `detail.rs`, `detail_tests.rs`, and the legacy renderer in `detail_header.rs`. Keep `chip_divider_line` (a live section rule, later the framework's `rule`) and move it next to its callers. Remove the `e` binding, its flag, its help row, and its documentation. Update `docs/tui-detail-mockup.md`, and mark any ADR that describes the removed model as superseded in `docs/adr/README.md`.

TUI changes:
- Consistency wins: the code the TUI runs and the code in the tree are the same code
- Regressions or behavior changes: the inert `e` key disappears from the keymap and the help page; it did nothing visible

Tests and checks: Every deleted test is either deleted with the code it covered or rewritten against the retargeted reads; none is dropped silently. `cargo test --all-targets --all-features` passes. `conspectus tui --snapshot` frames are identical before and after for the Sessions, Mux, and Pinned views.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 The sectioned detail model (`detail.rs`, its tests, and the legacy renderer in `detail_header.rs`) is deleted, and `chip_divider_line` is kept next to its callers
- [ ] #2 Production code that read `.kind` and `ambiguous_muxes_for_group` is retargeted, with its test coverage moved or rewritten rather than dropped
- [ ] #3 The `e` binding, its flag, its help row, and its documentation are removed, and the keymap drift tests pass
- [ ] #4 `docs/tui-detail-mockup.md` and the ADR index say the model is gone
- [ ] #5 `conspectus tui --snapshot` frames for three views match the frames from before the change
<!-- AC:END -->

## Definition of Done
<!-- DOD:BEGIN -->
- [ ] #1 `just check` passes, or `git diff --check` for docs-only changes
- [ ] #2 Docs and ADRs are updated when behavior, architecture, or workflow change
<!-- DOD:END -->
