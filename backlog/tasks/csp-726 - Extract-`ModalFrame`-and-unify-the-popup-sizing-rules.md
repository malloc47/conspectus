---
id: CSP-726
title: Extract `ModalFrame` and unify the popup sizing rules
status: Done
assignee:
  - '@claude'
created_date: '2026-10-27 20:26'
updated_date: '2026-10-28 17:51'
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
- [x] #1 `ModalFrame` provides named sizing policies, the themed frame, and one title style, and every modal uses it
- [x] #2 The 4 hand-built frames, the 11 `KnownSize` impls, `LinesBody`, and the 13 ad hoc sizing rules are removed
- [x] #3 `tui-popup` is removed from the dependency tree
- [x] #4 The layout consistency checks pass for every modal at 5 widths and 2 heights
- [x] #5 Every visible change is listed in the final summary under **Regressions** and **Consistency wins**, and was checked with `conspectus tui --snapshot` at a normal and a narrow width
- [x] #6 The framework crate names no Conspectus type, string, theme field, or domain word, and the independence guard passes
- [x] #7 The control has a reference page in the framework docs: purpose, usage, key table, theme tokens used, and the house rules it follows
<!-- AC:END -->

## Definition of Done
<!-- DOD:BEGIN -->
- [x] #1 `just check` passes, or `git diff --check` for docs-only changes
- [x] #2 Docs and ADRs are updated when behavior, architecture, or workflow change
<!-- DOD:END -->

## Implementation Plan

<!-- SECTION:PLAN:BEGIN -->
1. Snapshot mode reaches every modal (rename, picker, Clean up added); capture before frames of 35 modal states at 100x30, 80x24, 60x20, 40x12.
2. Inventory the 13 rules as they are in the code and reduce them to one formula (share, cap, min, margin per axis; content or fill height): clamp(min(cap, share of screen, screen - 2*margin), min, screen). Five named policies: COMPACT (prompts, checklists), MENU (menus, settings), FORM (forms), WIDE (readers, help, messages, picker), PALETTE (search).
3. Add `tuikit::frame`: `Extent`, `Fit`, `ModalSize` and its policies, `ModalFrame` (cleared surface, accent border, one plain title form with one space each side, padding), the `ModalBody` trait (measured once at the policy's inner width), and a `Lines` body that follows a cursor span. Unit tests, docs page, module map row; leave the pane frame seam.
4. Move every modal onto it: help pages, value modal, Messages, search, picker, text input, multi-select, Controls, the three forms, row/mux/worktree menus, Clean up. Delete popup_frame.rs (centered_rect, themed_popup, LinesBody), the KnownSize impls, form::modal_rect, the per-modal rect functions; drop tui-popup from Cargo.toml and Cargo.lock.
5. Tests: layout consistency checks for every modal, a one-title-style check, host tests that encoded old sizes updated deliberately.
6. After frames, explain each difference; ADR 0158, design.md, CHANGELOG, operator-review; full checks.

7. Rules as found in the code, and the policy each modal maps to: help `min(100,w-4).max(40)` x `(h-4).max(10)` -> WIDE; value modal `min(78,w-4).max(40)` x `min(28,h-2).max(10)` -> WIDE; Controls `min(64,w-4).max(40)` x `(n+2).clamp(8,h)` -> MENU; text prompt `min(60,w-4).max(20)` x 3 -> COMPACT; checklist `min(60,w-4).max(20)` x `(n+2).clamp(3,h-4)` -> COMPACT; picker `min(80,w-4).max(30)` x `(h-4).max(8)` (its 20-row cap never applied) -> WIDE; search `(85% w).clamp(60,200).min(w-2)` x `(80% h).clamp(12,50).min(h-2)` -> PALETTE; Messages `(w-4).clamp(40,110)` x `(h-2).max(10)` -> WIDE; row menu `72.min(w)` x `(n+2).max(8).min(h)`, mux menu `62.min(w)` x `max(10)`, worktree menu `56.min(w)` x `max(5)`, empty Clean up `68.min(w)` x `(n+2).min(h)` -> MENU (Clean up with records is the checklist, COMPACT); forms `min(76|76|84,w-4).max(44)` x `(n+2).clamp(10|6|10,h)` -> FORM.
<!-- SECTION:PLAN:END -->

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
Commits: 1dd5ebb (snapshot mode reaches rename, picker, Clean up), e8c79ec (tuikit::frame), 6996848 (host migration), c1a96ab (drop tui-popup), 6e19e34 (ADR 0158 and docs).
Before/after frames: /tmp/tuifw-baseline/frames-726-before and -after, 35 modal states x 100x30, 80x24, 60x20, 40x12 (140 each; 31 identical). Every difference is a size, position, title-form, or wrap change listed in the final summary; compared with /tmp/tuifw-baseline/cmp726.py, which reports each accent-bordered frame's rect and title.
Validation: cargo fmt --all -- --check (pass); cargo clippy --workspace --all-targets --all-features -D warnings (pass); full suite via run-tests.sh 3755 passed, 0 failed, 1 ignored (pass); RUSTDOCFLAGS='-D warnings' cargo doc --workspace --no-deps (pass); git diff --check (pass); cargo test -p tuikit including independence, raw_styles, glyph_literals, documentation (pass). `just check` itself was not run because it runs the suite without the tmux isolation the brief requires; its parts were run as listed.
<!-- SECTION:NOTES:END -->

## Final Summary

<!-- SECTION:FINAL_SUMMARY:BEGIN -->
Added `tuikit::frame` (ADR 0158): `ModalSize` (an `Extent` per direction: share, cap, minimum, margin, the margin giving way to the minimum and the screen bounding both; a `Fit` for the height) with five named sizes, COMPACT 60, MENU 64, FORM 80, WIDE 100 x full height, PALETTE 85% x 80%; `ModalFrame` (cleared rect on `surface`, accent border, one title form, optional padding); `ModalBody`, measured once at the policy's inner width; and `Lines`. Every modal draws in it: prompts and checklists COMPACT; row, mux, worktree menus, Clean up, Controls MENU; the three forms FORM; help pages, value modal, Messages, picker WIDE; search PALETTE. Removed `popup_frame.rs` (`centered_rect`, `themed_popup`, the dead `LinesBody`), the four hand-built frames, the 11 `KnownSize` impls, `form::modal_rect`, the per-modal rect functions, Controls' height budget, and `tui-popup` (with `derive_setters` and an old `darling`) from Cargo.toml and Cargo.lock. Title choice: plain (the border's accent, regular weight, one space each side), the form 12 of 14 titles had. Snapshot mode can now open rename, the picker, and Clean up. Page: `crates/tuikit/docs/controls/frame.md`; the pane frame is a documented seam.

Verified: fmt, clippy -D warnings, full isolated suite (3755 passed, 0 failed, 1 ignored), cargo doc -D warnings, git diff --check, the tuikit guards. The layout checks (nothing escapes the frame, selection moves no text, no cut words) pass for every modal at 5 widths x 2 heights, and a new check asserts one accent frame and one title form for every modal. Before/after `--snapshot` frames of 35 modal states at 100x30, 80x24, 60x20, 40x12 are in `/tmp/tuifw-baseline/frames-726-*`. Updated snapshots: the two row-menu frames, two mux launch forms, three viewer help pages.

**Regressions**
- Row menu 72 -> 64 columns (subtitles wrap sooner); Clean up notice 68 -> 64; pin edit form 84 -> 80.
- Menus, forms, and Controls stop a row short of the screen's top and bottom: on 24 rows a long menu or Controls shows two fewer rows and scrolls.
- Below 62 columns or 14 rows the search keeps its 60 x 12 minimum (full width at 60 columns, the whole 40 x 12 screen) instead of a one-cell margin.
- Menus on a 60-column terminal are 56 wide (they were flush at 60).
- The four hand-built menus move one cell when the space around them is odd (the odd cell goes before, as the other modals already did).
- The value modal and the picker are the help's size: wider on terminals over 82 columns, the value modal taller than 28 rows on tall ones, the picker two rows taller.

**Consistency wins**
- One frame and one title form for every modal; Messages and value titles lose their bold and doubled spaces; rename, checklist, and Clean up titles gain their spaces.
- Named sizes: mux menu 62 -> 64, worktree menu 56 -> 64, new mux and mux launch forms 76 -> 80; help pages two rows taller.
- Every modal keeps two columns and one row clear around it, so the header and status line stay visible.
- Menus hug their entries (the 8, 10, and 5 row floors are gone).
- Below 44 columns the forms lay out for the width they are drawn at, so key hints wrap instead of being cut; the empty Clean up wraps its sentence instead of cutting `[history.reap_after]`.
- `surface` paints modal backgrounds; no modal can draw unthemed.
<!-- SECTION:FINAL_SUMMARY:END -->
