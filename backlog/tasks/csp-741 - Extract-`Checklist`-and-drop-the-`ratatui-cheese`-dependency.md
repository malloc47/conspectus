---
id: CSP-741
title: Extract `Checklist` and drop the `ratatui-cheese` dependency
status: Done
assignee:
  - '@claude'
created_date: '2026-10-27 20:27'
updated_date: '2026-10-29 12:06'
labels:
  - tui-framework
  - tuifw-choice
milestone: m-25
dependencies:
  - CSP-729
  - CSP-727
  - CSP-730
documentation:
  - docs/plans/tui-framework/inventory/04-menus-pickers-dialogs.md
  - docs/adr/0152-tui-framework-dependency-ownership.md
priority: medium
type: enhancement
ordinal: 827000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
The multi-select is a thin shim over `ratatui_cheese::multi_select` that keeps a per-frame mirror state and a hand-written `Clone`, and the Clean up modal is another checklist. The cell is silently cut at the edge without an ellipsis, and the list has no select-all and no selected count. ADR 0152 replaces the shim with in-tree code (inventory slice 04).

Scope: Create `Checklist` with labels, preselection, disabled rows, select all and none, a selected count, and `on_confirm(indices)`. Rebuild the Controls sub-editors (harness, mux state, PR state) and the Clean up modal on it. Candidate labels and the record mapping stay in Conspectus. Remove `ratatui-cheese` from the dependency tree (the help use is removed by the help extraction).

TUI changes:
- Consistency wins: select all, select none, and a selected count; long labels end in an ellipsis instead of a hard cut; no mirror state or hand-written `Clone`
- Regressions or behavior changes: none expected

Tests and checks: Checklist tests for toggling, range, disabled rows, select all and none, and confirm. `--snapshot` of the Controls sub-editors and Clean up.

Blockers: The overlay contract, the selection extraction, and the help extraction (the other `ratatui-cheese` use).
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [x] #1 `Checklist` replaces the multi-select shim and the Clean up modal's list, with select all and none and a selected count
- [x] #2 Long labels end with an ellipsis
- [x] #3 `ratatui-cheese` is removed from the dependency tree
- [x] #4 Every visible change is listed in the final summary under **Regressions** and **Consistency wins**, and was checked with `conspectus tui --snapshot` at a normal and a narrow width
- [x] #5 The framework crate names no Conspectus type, string, theme field, or domain word, and the independence guard passes
- [x] #6 The control has a reference page in the framework docs: purpose, usage, key table, theme tokens used, and the house rules it follows
<!-- AC:END -->

## Definition of Done
<!-- DOD:BEGIN -->
- [x] #1 `just check` passes, or `git diff --check` for docs-only changes
- [x] #2 Docs and ADRs are updated when behavior, architecture, or workflow change
<!-- DOD:END -->

## Implementation Plan

<!-- SECTION:PLAN:BEGIN -->
1. Capture before frames of the Controls checklists (harness, mux state, PR state) and Clean up (all selected, several deselected, scrolled) at 100x30, 60x20, 40x10 into /tmp/tuifw-baseline/frames-741-before.
2. tuikit: add the `checklist` module: `Checklist<M>` (an `Overlay<M, Cx>` for any context) over `Item`s (label, optional detail, checked, enabled), `on_confirm(Vec<usize>) -> Outcome<M>`, `CHECKLIST_SIZE`; the shared cursor look, wrapping and stepping over disabled items; a window kept between draws with the shared scrollbar and a fixed footer; the selected count on the bottom border; labels cut with `…` in cells.
3. tuikit keys: extend `CHECKLIST_KEYS`/`ChecklistKey` with `a` select all, `n` select none (shown), and PgUp/PgDn, Home/End/g/G (aliases, house rule 12).
4. tuikit tests: toggling, disabled rows, select all/none, count, ellipsis, confirm/cancel/dismiss, wrap, scroll at 40x10 and 80x24, goldens, conformance; reference page docs/controls/checklist.md; architecture, README, keys page.
5. Host: Controls sub-editors become `Checklist<Vec<..>>` with one settle helper; Clean up becomes `clean_up::checklist(candidates, now) -> Checklist<Msg>` held by `Modal::CleanUp`, labels and the `ReapRecords` mapping stay in clean_up.rs; delete multi_select.rs and its tests, `CleanUpState`, `CleanUpWidget`, and the entries.rs impl; update consistency tests, ui tests, and the pantry ingredient.
6. Remove `ratatui-cheese` from Cargo.toml and Cargo.lock; confirm `cargo tree -i ratatui-cheese` finds nothing.
7. Docs: ADR 0173 (checklist keys, count, disabled rows), ADR index and amendments, design.md, operations.md, CHANGELOG.md, operator-review.md.
8. After frames, compare by category; fmt, clippy, full isolated suite, cargo doc, build examples, git diff --check; finalize the backlog task.
<!-- SECTION:PLAN:END -->

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
Slices: 0600eb8a (framework `Checklist<M>`, `Item`, `CHECKLIST_SIZE`, the new `CHECKLIST_KEYS` rows, tests, goldens, reference page; the old shim got a catch-all arm so the slice builds alone, checked with cargo check and clippy on that tree), f6874bb3 (Controls editors and Clean up on the control, shim and `ratatui-cheese` removed, pantry, consistency registry, operator guide, changelog), c170734b (ADR 0173, amendments to 0160 and 0031, design.md, operator review).
Line counts: the shim was 243 lines (172 code) over `ratatui-cheese`; the control is 603 lines (327 code: checklist.rs 169, body.rs 126, item.rs 32) and adds select all/none, the count, paging and ends, items that cannot change, the footer, and a kept window. Clean up went from 114 to 66 lines.
`cargo update -p ratatui-cheese` also re-resolved windows-sys; reverted and let `cargo metadata --offline` prune the lockfile instead (11 lines removed, nothing else). `cargo tree -i ratatui-cheese`: "did not match any packages".
The History consistency world has one unpinned record, so the registry's Clean up case is built from four candidates (a one-item list cannot show its cursor move or wrap).
Frames: /tmp/tuifw-baseline/frames-741-{before,after} at 100x30, 60x20, 40x10 (capture-741.sh); categories in frames-741-diff.txt.
<!-- SECTION:NOTES:END -->

## Final Summary

<!-- SECTION:FINAL_SUMMARY:BEGIN -->
Landed the framework's `Checklist<M>` (`tuikit::checklist`: `Checklist`, `Item`, `CHECKLIST_SIZE`), an `Overlay<M, Cx>` for any context over items that are data (label, detail, checked, can change). `Enter` hands the checked indices to the host's `on_confirm(Vec<usize>) -> Outcome<M>`; `Space` toggles, `a`/`n` select all/none (new rows in `CHECKLIST_KEYS`, with `PgUp`/`PgDn`, `Home`/`g`, `End`/`G` as aliases); the bottom border counts (`3 of 4 selected`); items that cannot change recede, keep their check, and are stepped over; rows are cut with `…` in cells; the generated footer keeps the bottom rows while the items scroll in a kept window. The Controls harness, mux state, and PR state editors are `Checklist<Vec<T>>`s; Clean up is `clean_up::checklist(..) -> Checklist<Msg>` held by `Modal::CleanUp` (labels and the `ReapRecords` mapping stay in the host). `multi_select.rs` (mirror state, hand-written `Clone`), `CleanUpState`, and `CleanUpWidget` are deleted; `ratatui-cheese` left Cargo.toml and Cargo.lock (`cargo tree -i ratatui-cheese`: no match). ADR 0173 (amends 0160, 0031); reference page crates/tuikit/docs/controls/checklist.md; design.md, operations.md, CHANGELOG.md, operator-review.md updated. Commits 0600eb8a, f6874bb3, c170734b.

Verified: checklist unit tests (toggling, disabled items, select all/none, count, ellipsis, confirm/cancel/dismiss, wrap, paging, scroll at 40x10 and 80x24, goldens, conformance); host tests for Controls and Clean up; `cargo fmt --check`, clippy `-D warnings`, the isolated suite (4197 passed, 0 failed, 1 ignored), isolated nextest (4197 passed, 1 skipped), `cargo doc -D warnings`, `cargo build --examples`, `git diff --check`, the independence guard. `conspectus tui --snapshot` before/after at 100x30, 60x20, 40x10 (/tmp/tuifw-baseline/frames-741-*).

Tests changed deliberately: Clean up's `q` answers `Dismiss` (closes the same way); the Clean up ui test checks the records through the confirm outcome; the conformance registry checks Clean up's footer and selection over four candidates; the checklist footer test reads the new table.

**Regressions**
- At 40x10 a checklist shows three items instead of five or six, because its three footer rows stay on screen; an overflowing list (Clean up) reserves the scrollbar column but draws no bar under five rows (the menus' seam), so its labels are cut one cell earlier.
- Every checklist is four rows taller at 100x30 and 60x20 (a blank row above the items, a blank row and the footer below), covering more of the screen under it.

**Consistency wins**
- `a` selects all and `n` selects none (new capability, in every checklist).
- The selected count on the bottom border (`0 of 5 selected`).
- The keys are shown: a footer generated from the checklist table (`↑/↓ move · Space toggle · a select all · n select none · Enter confirm · Esc cancel`), kept at the bottom on a short screen while the items scroll with the cursor; the list keeps its window between keys.
- A blank row above the items, as menus and dialogs have.
- `PgUp`/`PgDn`, `Home`/`End`, and `g`/`G` move the cursor (house rule 12).
- Long labels end with `…` through `tuikit::text` (a detail is cut before its label); the rows otherwise look as before (cursor gutter and highlight, `√` accent bold, `•`).
<!-- SECTION:FINAL_SUMMARY:END -->
