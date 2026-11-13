---
id: CSP-780
title: >-
  Find a stacked step's frame in the conformance suite when it covers the entry
  under it exactly
status: Done
assignee:
  - '@claude'
created_date: '2026-10-29 11:04'
updated_date: '2026-10-30 19:17'
labels:
  - tui-framework
milestone: m-25
dependencies: []
priority: low
ordinal: 866000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
The conformance suite (`tuikit::testkit::conformance`) finds a modal's frame by the border corners that differ between the screen with the modal and the screen after `Rig::close_top`. A step opened over a menu with `ModalStack::push_step` (ADR 0172) can sit at exactly the menu's rect, as a worktree dialog does when it has as many rows as the menu or both fill a 40x10 screen; then no corner differs and the checks report "draws no frame". CSP-740 worked around it in the host: its step cases use a worktree with three live sessions so the remove confirmation is taller than the menu, and the 40x10 body check compares with the whole flow closed. A suite that knew the rect each entry drew (the overlay page's "Rendered areas" seam) would check any step at any size.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [x] #1 The conformance checks find the frame of a step whose rect equals the rect of the entry under it
- [x] #2 The host step cases no longer depend on worktree facts chosen to size the step apart from the menu
- [x] #3 The overlay or testkit control page describes how a step's frame is found
<!-- AC:END -->

## Definition of Done
<!-- DOD:BEGIN -->
- [x] #1 `just check` passes, or `git diff --check` for docs-only changes
- [x] #2 Docs and ADRs are updated when behavior, architecture, or workflow change
<!-- DOD:END -->

## Implementation Plan

<!-- SECTION:PLAN:BEGIN -->
1. Add `testkit::buffer::covering_frame(with, without)`: the smallest frame `with` draws around every cell that differs from `without`, which is a modal's own frame when it is drawn exactly over another frame of the same look (no corner differs).
2. Route the conformance checks' frame lookups through `frame_of(case, ..)`: `frame_rect`, then for a `Case::step` only, `covering_frame`; a non-step modal whose frame cannot be told from the screen under it still draws no frame.
3. Tests: unit tests of `covering_frame` (exact cover, a retitled frame, nothing changed, nested and edge cases); a conformance case whose step covers its menu exactly at every size passes every check, and the same modal opened as no step fails "draws no frame".
4. Host: drop the three-session worktree chosen to size the dialogs apart (one live session now), and compare the 40x10 body check with the top modal closed, the status bar row kept, falling back to `covering_frame`.
5. Docs: testkit.md (how a step's frame is found), overlay.md seam note, `Case::step` docs; ADR 0194 and ADR 0164's status; operator-review rows.
<!-- SECTION:PLAN:END -->

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
Chose to infer the frame from the screen (the smallest frame around what changed) over the rendered-areas seam (an `Overlay::rect` every control and host overlay would implement): the suite needs only what is drawn, and a rect computed ahead of the draw could drift from the one drawn. Restricted the fallback to `Case::step` so a frameless modal inside a pane cannot pass on the pane's border. Evidence the host now relies on it: with the fallback disabled, the one-session worktree's remove dialog fails "draws no frame" at five suite sizes. The host's 40x10 body check also needed the status bar row copied over (it names the top modal's keys and lies outside every frame).
<!-- SECTION:NOTES:END -->

## Final Summary

<!-- SECTION:FINAL_SUMMARY:BEGIN -->
The conformance suite now finds the frame of a step drawn exactly over the entry it is a step of (commits 20f6f25d, ce3cfa16, 9c8c7fb0). `testkit::buffer::covering_frame(with, without)` is the smallest frame drawn around every cell that changed; the checks (`frame_of` in `testkit/conformance/checks.rs`) take it for a `Case::step` when `frame_rect` finds no differing corner, so a step that covers its menu exactly is checked at every size, while any other modal whose frame cannot be told from the screen under it still fails "draws no frame". The overlay contract is unchanged: no entry reports its rect (the rendered-areas seam stays open for mouse hit testing). ADR 0194 records the choice; `testkit.md` describes how a step's frame is found, and the `overlay.md` seam and `Case::step` docs point at it.

The host dropped its workaround: the worktree step cases use a worktree with one live session instead of three, and the 40x10 body check compares with the top modal closed (the status bar row copied over, as the suite's rig does) instead of the whole flow.

Verification: unit tests of `covering_frame` (an exact cover, a retitled frame, nothing changed, the smallest of nested frames, a change on the screen's edge); a conformance case whose step covers its menu at every size passes every check, and the same modal opened as no step fails "draws no frame". With the fallback disabled, the host's conformance tests over the one-session worktree fail "draws no frame" for the remove dialog at 60x20, 60x40, 80x20, 80x40, and 120x40; with it they pass. fmt, clippy `-D warnings`, the isolated suite (4746 passed, 0 failed, 1 ignored), rustdoc `-D warnings`, `cargo build --examples`, and `git diff --check` pass.

**Regressions**
- None: the change is in the test kit's checks and the host's tests; nothing on screen changes.

**Consistency wins**
- None visible. Every step is now held to the frame checks at every size, whatever its size beside the entry under it.
<!-- SECTION:FINAL_SUMMARY:END -->
