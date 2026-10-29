---
id: CSP-739
title: Extract `ActionMenu<M>`
status: Done
assignee:
  - '@claude'
created_date: '2026-10-27 20:27'
updated_date: '2026-10-29 09:43'
labels:
  - tui-framework
  - tuifw-choice
milestone: m-25
dependencies:
  - CSP-729
  - CSP-727
documentation:
  - docs/plans/tui-framework/inventory/04-menus-pickers-dialogs.md
  - docs/adr/0151-tui-framework-control-set-and-house-rules.md
priority: medium
type: enhancement
ordinal: 825000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Four menus are one control: the row menu (`.`), the mux menu (`m`), the worktree menu's list, and the pin unpin menu. Each has its own heading and width (the mux menu is hand-framed at 62 cells), the row menu cannot scroll, and the committed `history_row_menu` snapshot already loses its footer at 24 rows. One entry, Forget, confirms by an inline "press Enter again"; the others do not (inventory slice 04).

Scope: Create `ActionMenu<M>` with entries `{label, detail, shortcut, enabled, confirm: None | Inline, outcome: M}`, scrolling, generated footer, and the shared selection and frame. Turn the mux menu into data fed to it. The entry policy (`row_menu_for`, `pin_management_entries`) and the `Msg` mapping stay in Conspectus. Apply house rules 1 to 5.

TUI changes:
- Consistency wins: menus scroll and keep their footer on short terminals; one heading and width policy; `q` closes a menu; an inline-confirm entry shows what Enter will do
- Regressions or behavior changes: the mux menu's heading and width change to the shared policy; menus wrap at the ends

Tests and checks: Menu tests for scrolling, disabled entries, inline confirm, and shortcuts. `--snapshot` of each menu at 80x24 and 40x10. The `consistency_tests/menus.rs` entity-equivalence checks pass.

Blockers: The overlay contract and the selection extraction.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [x] #1 `ActionMenu<M>` replaces the row menu, the mux menu widget, the worktree menu's list, and the pin unpin menu
- [x] #2 The menu scrolls and keeps its generated footer at 40x10 and 80x24
- [x] #3 An entry can require an inline second Enter, and the entry says so
- [x] #4 The mux menu widget file is deleted and its two verbs are data
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
1. Framework: add `tuikit::action_menu` with `ActionMenu<M>` (an `Overlay<M, Cx>` for every context), `Entry<M>` {label, detail, shortcut, enabled, confirm, outcome} and `Confirm` {None, Inline}. Keys from the standard `MENU_KEYS` (wrap, Ctrl-N/P, q/Esc/Ctrl-C close silently); a shortcut is an accelerator resolved after the table, and `shadowed_shortcuts` reports one the table or an earlier entry takes. Disabled entries are drawn in the unavailable style and skipped by the cursor; neither Enter nor their shortcut commits them. An inline-confirm entry arms on the first Enter (moving disarms) and its row then says what Enter will do, with the key taken from the table. Layout: blank row, entry rows through `selection::row` (an optional key column, the label cut with an ellipsis, details wrapped and indented), a footer generated from `MENU_KEYS` that stays at the bottom; the entries scroll in a kept `ScrollWindow` (minimal motion) with the shared scrollbar gutter; the frame is `ModalSize::MENU`.
2. Framework tests: scrolling (cursor always visible, footer kept at 40x10 and 80x24), disabled entries, inline confirm, shortcuts, wrap, close keys, a style golden, and the conformance suite over the menu. Reference page `crates/tuikit/docs/controls/action_menu.md`; architecture map and README.
3. Host: `row_menu_for`, `pin_management_entries`, `menu_title`, the unpin menu, and a new `actions::mux_menu` build `ActionMenu<Msg>`; the Pin entry gets the `p` shortcut (its detail drops `(p)`), Forget stays inline-confirm. Delete `row_menu.rs` and `mux_menu.rs` (the mux verbs become data; its in-body heading goes). Modal variants hold `ActionMenu<Msg>`; their `Overlay` impls go.
4. Host: the worktree menu's list level is an `ActionMenu<WorktreeAction>` (gains the menu footer); the mode machine stays for CSP-740.
5. Tests: update host tests and insta snapshots deliberately; add a consistency check that a menu shortcut does what the same key does in the main view, and that no shortcut is shadowed; register footers for the worktree list.
6. Capture before/after `--snapshot` frames of every menu at 100x30, 60x20, 40x10 into /tmp/tuifw-baseline/frames-739-*; classify the differences.
7. ADR 0171, ADR index and amended ADRs, operations.md, CHANGELOG, operator-review rows; run fmt, clippy, the isolated suite, rustdoc, diff check; close the task.

8. (Added while implementing) The mux menu's entries take `n` and `N`, the keys that open the same forms from the list; a consistency check (fixture routing for the row menu, the snapshot driver for the mux menu) holds every shown shortcut to what its key does on the list.
<!-- SECTION:PLAN:END -->

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
Landed in four commits: e0f2c0b3 (framework `action_menu` module, tests, goldens, conformance, page), b75acd97 (row, mux, and unpin menus on ActionMenu; `row_menu.rs`, `row_menu_tests.rs`, `mux_menu.rs` deleted; shortcut checks), d343ca3b (worktree list as an ActionMenu level; registry footer), 49945e44 (ADR 0171, ADR 0096 amendment, design, operations, changelog, operator review).

Decisions: disabled entries are stepped over and never committed (drawn in the unavailable style, detail plain); shortcuts sit in a key column after the gutter, resolved after `MENU_KEYS`, with `shadowed_shortcuts` as the check; the armed note is generated (`press Enter again to confirm`) and joined with the separator mark; `Esc` closes an armed menu; the menu keeps its scroll window and draws the shared scrollbar when it overflows.

Validation: `bash /tmp/tuifw-baseline/run-tests.sh` exit 0, 4142 passed, 0 failed, 1 ignored (no flakes this run); fmt check, clippy (workspace, all targets and features, -D warnings), `RUSTDOCFLAGS='-D warnings' cargo doc --workspace --no-deps`, `git diff --check`, and the independence guard all pass. Before/after `--snapshot` frames of every menu at 100x30, 60x20, and 40x10 in /tmp/tuifw-baseline/frames-739-{before,after} (diff in frames-739-diff.txt), plus after frames at 80x24. Filed CSP-779 for the scrollbar column kept where no bar can draw.
<!-- SECTION:NOTES:END -->

## Final Summary

<!-- SECTION:FINAL_SUMMARY:BEGIN -->
`ActionMenu<M>` (`tuikit::action_menu`, with `Entry<M>` and `Confirm`) is the one action menu: an `Overlay<M, Cx>` for every context over entries `{label, detail, shortcut, enabled, confirm, outcome}`, keys and footer from `MENU_KEYS`, wrapping, silent close on `Esc`/`q`/`Ctrl-C`, the shared selection look, the `MENU` frame, a fixed footer over entries scrolled in a kept window with the shared scrollbar. Disabled entries are drawn receding, stepped over by the cursor, and never committed; an inline-confirm entry arms on the first press and its row says `press Enter again to confirm`; shortcuts sit in a key column and `shadowed_shortcuts` finds ones no key reaches. Page `crates/tuikit/docs/controls/action_menu.md`; ADR 0171 (amends ADR 0096).

The row menu, the unpin menu, and the mux menu are `ActionMenu<Msg>` built in the host (`row_menu_for`, `pin_management_entries`, `menu_title`, `unpin_entry`, new `actions::mux_menu`); `row_menu.rs`, its tests, and `mux_menu.rs` are deleted. The worktree menu's list is an `ActionMenu<WorktreeAction>` level; its mode machine stays for CSP-740. Tests updated on purpose: the `history_row_menu` snapshot (now shows the scrollbar), the host tests reading `.msg`/`.confirm`, the worktree tests (`actions()`, `&mut` widget).

Verified with 30 framework unit tests (scrolling with the footer kept at 40x10 and 80x24, disabled entries, inline confirm, shortcuts, wrap, close keys, two style goldens, the conformance suite) and 3 doctests; new host checks that a menu shortcut does what its key does on the list and that no shortcut is shadowed; `consistency_tests/menus.rs` passes; the isolated full suite (4142 passed, 0 failed, 1 ignored); fmt, clippy, rustdoc with `-D warnings`, `git diff --check`. Frames: /tmp/tuifw-baseline/frames-739-{before,after} at 100x30, 60x20, 40x10 (and 80x24 after).

**Regressions**
- A menu with a shortcut (the row menu of an unpinned row, the mux menu) moves its labels and details three cells right, so at 40 columns long labels are cut sooner (`Forget · press Enter again to c…`).
- A menu whose entries overflow lays them out one cell narrower for the scrollbar; where the entries get fewer than five rows (40x10) the column is kept and no bar is drawn (CSP-779).
- The worktree menu's list is three rows taller (a blank row and its footer), so at 40x10 it shows three of its entries at a time and scrolls.
- The armed Forget's `—` became `·`.

**Consistency wins**
- The mux menu loses its `Mux actions` heading (one title form) and keeps its footer at 40x10, where it used to lose it.
- The worktree menu's list shows the generated `↑/↓ move · Enter pick · Esc close` and scrolls instead of cutting its last entries.
- Overflowing menus show the shared scrollbar, and a scrolled menu moves only as far as the cursor needs instead of keeping it on the last line.
- Keys are shown in a key column and work: `p` pins from the row menu (its detail no longer says `(p)`), `n` and `N` open the same forms from the mux menu as from the list.
- The armed entry's words are generated from the key table and are the same in every menu.
- Unchanged: the unpin menu, and the pin row menu where it fits (100x30, 80x24).
<!-- SECTION:FINAL_SUMMARY:END -->
