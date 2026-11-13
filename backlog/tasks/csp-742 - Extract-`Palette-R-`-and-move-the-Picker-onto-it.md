---
id: CSP-742
title: Extract `Palette<R>` and move the Picker onto it
status: Done
assignee:
  - '@claude'
created_date: '2026-10-27 20:27'
updated_date: '2026-10-29 12:59'
labels:
  - tui-framework
  - tuifw-choice
milestone: m-25
dependencies:
  - CSP-733
  - CSP-729
  - CSP-727
  - CSP-723
documentation:
  - docs/plans/tui-framework/inventory/04-menus-pickers-dialogs.md
  - docs/plans/tui-framework/inventory/05-search-help-viewers.md
priority: medium
type: enhancement
ordinal: 828000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
The Picker (retargeting a pin, search history), quick select, and content search are one control: a query line, a ranked list with a cursor, a status row, a generated footer, and an outcome. The Picker draws no caret and a dead height cap; quick select draws no caret either. The backlog's command palette (CSP-392) would be a third instance (inventory slices 04 and 05, section 6).

Scope: Create `Palette<R: PaletteRow>` with a `LineEdit` query, a scope strip in the title, a ranked list using the shared selection, a status row, a generated footer, an options pane hook, and a debounce contract for slow providers. Move the Picker onto it first, as the smallest instance. Ranking, history items, and the retarget candidates stay in Conspectus. Leave the seams for a command palette instance and for a row preview.

TUI changes:
- Consistency wins: the query line shows a caret; one list look; Ctrl-N and Ctrl-P work the same wherever the palette is used
- Regressions or behavior changes: the Picker's height and size follow the shared policy

Tests and checks: Picker tests ported unchanged. `--snapshot` of the Picker at 80x24 and 40x10.

Blockers: `LineEdit`, the overlay contract, selection, and text.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [x] #1 `Palette<R>` provides the query, scope strip, ranked list, status row, footer, and options hook
- [x] #2 The Picker is a `Palette` instance and its dead height cap is gone
- [x] #3 The documented seams for a command palette instance and a row preview exist
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
1. tuikit keys: add the standard palette tables, `PALETTE_KEYS` (`PaletteKey`: Ctrl-C dismiss, arrows and Ctrl-P/Ctrl-N move, PgUp/PgDn page as aliases, Enter pick, Esc close; `q`, `j`, `k` type) and `PALETTE_OPTION_KEYS` (`PaletteOptionKey`, the options pane: the search options rows), with tests and keys.md rows.
2. tuikit palette module: `Palette<R, M, A = PaletteKey>` implementing `Overlay<M, Cx>`: `LineEdit` query drawn by `TextField` (caret), optional scope strips in the title (`SegmentedTitle`, the first cycled by `PaletteKey::Scope`), a count, a ranked list (host ranker `Fn(&str, &[R]) -> Vec<usize>` or host-fed rows via `set_rows`), shared selection rows, wrap, PgUp/PgDn, a kept `ScrollWindow` and the shared scrollbar, a status row, empty notes, a generated muted footer, an options pane (`Options`: named on/off options drawn in the list's place, own table), a `Debounce` timer (`mark`, `take_due(now, force)`), `PaletteAction` so a host table over its own enum drives it (`host_action` for the host's own keys), `PaletteRow` (cells for a width, cursor, query, theme; haystack), `Item<T>` and `every_word`. Unit tests in palette_tests.rs, goldens, and a conformance-suite case. Default size `ModalSize::PALETTE`.
3. tuikit docs: controls/palette.md (purpose, usage, key table, tokens, house rules, seams: command palette from an action table, row preview, column plan, progress formatter), architecture row landed, README module list.
4. Host: the Picker becomes `Palette<Item<String>, Msg>` built by `picker::retarget` and `picker::search_history`; `PickerPurpose`, `PickerState`, `PickerWidget`, `PICKER_KEYS`, and the entries impl go; callers, conformance registry (PALETTE_KEYS, footer), table registry, ui/consistency tests updated; picker tests ported. Dead height cap verified gone. CHANGELOG and operations.md entries.
5. ADR 0174 (palette), amend ADR 0158 (picker size) and 0160 (tables), README index, design.md paragraph, operator-review rows.
6. Before/after `--snapshot` frames of the Picker at 100x30, 60x20, 40x10 in /tmp/tuifw-baseline/frames-742-*; classify differences.
7. fmt, clippy, full isolated suite, rustdoc -D warnings, build examples, git diff --check; backlog final summary with Regressions and Consistency wins.
<!-- SECTION:PLAN:END -->

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
- e1367655 feat(tuikit): `tuikit::palette` (`Palette<R, M, A = PaletteKey>`, `PaletteRow`, `RowContext`, `Item<T>`, `every_word`, `Options`, `Debounce`, `Count`, `PaletteAction`, `PALETTE_SIZE`) and the standard `PALETTE_KEYS`/`PALETTE_OPTION_KEYS`; 53 unit tests, two goldens, a conformance-suite case, doctests; page `crates/tuikit/docs/controls/palette.md`.
- 137d2b91 refactor(tui): the picker is `Palette<Item<String>, Msg>` built by `picker::retarget` and `picker::search_history`; `PickerPurpose`, `PickerState`, `PickerWidget`, the entries impl, and `PICKER_KEYS` deleted; registries hold the picker to `PALETTE_KEYS` and its footer; picker tests ported with their assertions unchanged (plus three new); CHANGELOG and operations.md.
- 9d93be6e docs: ADR 0174, amendments to ADR 0158 and 0160, design.md, operator-review rows.
- The dead height cap (D20) was already removed by CSP-726; verified nothing of it remains (the palette size fills its extent and the body measures only for content-fit sizes).
- Design choices for CSP-743: host tables over `SearchKey` keep working through `PaletteAction` + `host_action`; scope strips, the options pane, the status row, notes, and the debounce map onto the search overlay's parts. Quick select would inherit the edit cursor rule, the kept window, and the scrollbar.
- Frames: /tmp/tuifw-baseline/frames-742-{before,after} (11 scripts x 100x30, 60x20, 40x10; `capture-742.sh`, a showcase fixture with 30 more tmux sessions for scrolling); diff in frames-742-diff.txt.
- Checks: fmt, clippy (workspace, all targets, all features, -D warnings), full isolated cargo test (4252 passed, 0 failed, 1 ignored), rustdoc -D warnings, cargo build --examples, git diff --check, independence guard: all pass.
<!-- SECTION:NOTES:END -->

## Final Summary

<!-- SECTION:FINAL_SUMMARY:BEGIN -->
Landed `tuikit::palette`: `Palette<R, M, A = PaletteKey>` implementing `Overlay<M, Cx>`, with a `LineEdit` query drawn by `TextField` (caret), scope strips in the title (the first switched by `PaletteKey::Scope`) and a count, results ranked by the host (`ranked_by` with the built-in `every_word`, or batches via `set_rows`) drawn through `selection::row` and cut with `…`, wrap, `PgUp`/`PgDn`, a kept `ScrollWindow` with the shared scrollbar, a status row, empty notes, an options pane (`Options`, `PALETTE_OPTION_KEYS`), a `Debounce` the host polls (`take_due(now, force)`), a footer generated from `PALETTE_KEYS` or a host table over its own enum (`PaletteAction`, `host_action`), and `on_pick` as the outcome. `PaletteRow` lays a row out for a width, cursor, query, and theme. The Picker (retarget and search history) is the first instance (`picker::retarget`, `picker::search_history`); `PickerPurpose`, `PickerState`, `PickerWidget`, and `PICKER_KEYS` are gone, and the candidates and history items stay in Conspectus. The dead height cap was already gone (CSP-726). Seams on `crates/tuikit/docs/controls/palette.md`: a command palette from a key table, a row preview, a column plan. ADR 0174 (amends 0158 and 0160).

**Consistency wins**
- The picker shows its keys (`↑/↓ move · Enter pick · Esc close`); it drew no footer.
- `PgUp`/`PgDn` move by the rows shown (they did nothing).
- One list look: long candidates end in `…` instead of being clipped at the frame; an overflowing list shows the shared scrollbar; the window is kept, so moving back up a scrolled list no longer drags it.
- One cursor rule: each edit puts the cursor on the first match (it was clamped to the narrowed list).
- `Ctrl-N`/`Ctrl-P` come from the standard palette table every palette shares; `Ctrl-C` answers `Dismiss` as the framework's other controls do.
- The query's caret (from CSP-733) verified in the ANSI frames.

**Regressions**
- Size: the picker takes the palette size, the search overlay's: 85x24 at 100x30 with the panes showing around it (was 96x28), 60 wide edge to edge at 60x20 (was 56), unchanged at 40x10.
- At 40x10 the footer takes a row, so six candidates show instead of seven.
- The title's count sits two cells after the title (one more).

Verified with `conspectus tui --snapshot` frames of the retarget picker (empty, filtered, no match, caret, moved then typed, and a 37-candidate list at the top, wrapped up, moved up, paged, filtered) at 100x30, 60x20, and 40x10 (`/tmp/tuifw-baseline/frames-742-*`); 53 palette unit tests, goldens, a conformance case, doctests; ported picker tests; fmt, clippy -D warnings, the full isolated suite (4252 passed, 0 failed, 1 ignored), nextest, rustdoc -D warnings, `cargo build --examples`, `git diff --check`, and the independence guard.
<!-- SECTION:FINAL_SUMMARY:END -->
