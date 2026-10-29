---
id: CSP-746
title: Move the value modal and the help page body onto `Pager`
status: Done
assignee:
  - '@claude'
created_date: '2026-10-27 20:27'
updated_date: '2026-10-29 18:42'
labels:
  - tui-framework
  - tuifw-reading
milestone: m-25
dependencies:
  - CSP-745
  - CSP-712
documentation:
  - docs/plans/tui-framework/inventory/05-search-help-viewers.md
priority: medium
type: enhancement
ordinal: 832000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
The value modal (read-only text with a `y` copy payload) and the help page body are pagers: the modal sets an unclamped `u16::MAX` offset on `G`, and both page by a fixed 8 lines. They sit apart from the viewers' pager although they are the same control in Modal mode (inventory slice 05).

Scope: Re-host both on `Pager` in Modal mode: the value modal as a content adapter with the copy hook, the help body as generated rows. The rebuild-plan `$ ` highlighting stays as a small content hook in Conspectus.

TUI changes:
- Consistency wins: `G` and paging behave like the viewers; one footer and frame
- Regressions or behavior changes: page size changes from 8 lines to the viewport

Tests and checks: Value modal and help tests ported. `--snapshot` of both at 80x24 and 40x10.

Blockers: `Pager`.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [x] #1 The value modal and the help body use `Pager` in Modal mode, and their own scroll code is removed
- [x] #2 The value modal's copy hook still works and is covered by a test
- [x] #3 `G` lands on the last page and never renders a blank body
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
1. Footer-fit rule (the open question from CSP-745): keep the help page's rule for every framed pager. A framed pager's find chip or prompt and its footer keep the frame's last rows, laid out at the text width, leaving the body at least one row; footer rows that do not fit there are the footer's first rows (the scroll keys), and they scroll with the body after its last row, so the keys that leave stay in view and no key is lost. Implement it in `pager/body.rs` (`Framed`), with the rows that spill drawn by `draw_body` after the content's.
2. `tuikit::help`: `HelpPage` becomes an owned pager content (`HelpPage<M = ()>`: sections, title, the layout cached by width; `lines(theme, width)` stays the layout); `HelpPage::pager(table)` is the page in `Modal(HelpPage::SIZE)` resolving `table`. Delete `HelpScroll`, `HelpPage::render`, `scrolled`, `footer`, and `footer_lines`: scrolling, the scrollbar, and the footer are the pager's.
3. `tuikit::pager`: the `?` level is a nested `Pager<HelpPage>` over `PAGER_HELP_KEYS`, whose `?` becomes another `Close` (footer unchanged: `Esc/? back`); `help_scroll` becomes `help_page`.
4. Conspectus value modal: `ValueText` content (wrapped value rows with the `$ ` command-line style as the host's hook, the label as title, `copy` returning `Msg::Copy` from `copy_payload`), `ValueModal = Pager<ValueText>` in `Modal(WIDE)` with `VALUE_MODAL_KEYS`; delete `ValueModalState`'s scroll, `handle_key`, `fit`, `ValueModalWidget`, `ValueModalOutcome`, and its `Overlay` impl. Callers (`open_value_modal_for_cursor`, the rebuild plan openers in `runtime/launch.rs`) build it through `value_modal::open` and `value_modal::plan`.
5. Conspectus keymap help: `KeymapHelp = Pager<HelpPage<Msg>>` built from the keymap sections and legends with `HELP_KEYS` (`?` another close); delete `HelpOverlayState`, `HelpOverlayWidget`, `HelpOutcome`, and its `Overlay` impl. The transcript viewer's help page holds a `Pager<HelpPage>` its reducer routes the scroll messages to (until CSP-748 ports the viewer).
6. Port the tests: value modal, keymap help, viewer help, consistency (help, scrolling, conventions, layout), app tests, tuikit help and pager tests; add tests for the copy hook, `G` on the last page with no blank body, the footer-fit rule, and the nested help level.
7. Docs: pager.md (modes, footer rule, Ports final state), help.md (the page is pager content), keys.md, architecture.md; a new ADR for the footer rule and the help page as content; ADR index; operator-review rows; operations.md and CHANGELOG if visible; design.md if needed.
8. Frames: before/after `--snapshot` of the value modal (short, long, wrapped, end, paging) and the keymap help (top, scrolled, end) at 100x30, 60x20, 40x10; the rebuild plan through a throwaway render; classify the differences.
9. Checks: fmt, clippy, the isolated full suite, cargo doc with -D warnings, cargo build --examples, independence guard, git diff --check.
<!-- SECTION:PLAN:END -->

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
Footer-fit decision (the CSP-745 open question): every framed pager now follows the help page's rule. The prompt or find chip row stays, the footer keeps its last rows (the keys that leave), and its first rows scroll with the body after the content's last row; the rows under the body are laid out at the text width beside the scrollbar. `Below::fit` in `crates/tuikit/src/pager/body.rs`; ADR 0178 decision 3.

`HelpPage<M = ()>` is now the pager content itself (owned sections and title, layout cached per width); `HelpPage::pager(table)` frames it at `HelpPage::SIZE`. `HelpScroll`, `render`, `scrolled`, `footer`, and `footer_lines` are gone, and a pager's `?` level is a nested `Pager<HelpPage>` over `PAGER_HELP_KEYS`, whose `?` is now `Close`. One scroll implementation sits under every reader.

Host: `KeymapHelp = Pager<HelpPage<Msg>>` (`help::open(theme)`, sections built when it opens); `ValueModal = Pager<ValueText>` (`value_modal::open`, `ValueText::new`/`plan`; the `$ ` command style stays in `ValueText`'s layout; `copy` returns `Msg::Copy`). Deleted: `HelpOverlayState`, `HelpOverlayWidget`, `HelpOutcome`, `ValueModalState`'s scroll/`fit`/`handle_key`, `ValueModalOutcome`, `ValueModalWidget`, and both `Overlay` impls in `modal/entries.rs`. The transcript viewer's help is a boxed `Pager<HelpPage>` in `ViewerState` until CSP-748 (generated from `VIEWER_KEYS` on first draw; the reducer routes movement keys to its scroll calls). The `pantry` example follows.

Test expectation changes (mechanical): `q` and `Ctrl-C` answer `Dismiss` instead of `Close` in the value modal and the keymap help (same effect for an entry that is no step); the value modal's footer text in the plan test now reads `j/k/PgDn/PgUp scroll · y copy · Esc/q/o close`; the viewer-help key mapping test maps `?` (now `Close` on the help level) to the viewer's `ToggleHelp`; scroll tests that read `HelpScroll` offsets now draw the pager and read its window.

Frames: /tmp/tuifw-baseline/frames-746-{before,after} (87 frames: value modal short, long, scrolled, paged, half page, end, end then up, copy, closing keys, `?` and `/`, and the rebuild plan through a throwaway render; keymap help top, scrolled, paged, half page, end, end then up, closing, `/`; at 100x30, 60x20, 40x10). Diff in frames-746-diff.txt: 45 identical (every keymap help frame, styles included), 42 changed, all value modal, in four categories: footer order, footer look (dim instead of the inert color), footer wrapping when narrow, and the scrollbar with the text a cell narrower when the value overflows.

Follow-up filed: CSP-784 (forms, dialogs, checklists, and palettes fit an over-tall footer each their own way).
<!-- SECTION:NOTES:END -->

## Final Summary

<!-- SECTION:FINAL_SUMMARY:BEGIN -->
The value modal and the keymap help page now run on the framework `Pager` in Modal mode, so every reader scrolls, pages, finds its end, fits its footer, and closes the same way.

Framework: `tuikit::help::HelpPage<M = ()>` is a `PagerContent` (owned sections and title, laid out on the one grid per width, title as header, no keys). `HelpPage::pager(table)` frames it at `HelpPage::SIZE` over a host table. `HelpScroll` and the page's own drawing are removed. A pager's `?` level is a nested `Pager<HelpPage>` over `PAGER_HELP_KEYS`, whose `?` is now `Close` (footer unchanged). Footer-fit decision (open since CSP-745): a framed pager keeps the prompt or find-chip row and the footer's last rows (the keys that leave), and scrolls the footer's first rows with the body, the help page's rule, now applied to every framed pager. Rows under the body are laid out at the text width. ADR 0178; reference pages `pager.md` (Ports: long value and help page built) and `help.md`.

Host: `ValueModal = Pager<ValueText>` with `VALUE_MODAL_KEYS`. It wraps the value, keeps the rebuild plan's `$ ` accent as the host's hook in its layout, and `y` is the copy hook returning `Msg::Copy`. `KeymapHelp = Pager<HelpPage<Msg>>` with `HELP_KEYS`. Their state, widgets, outcomes, key handling, and overlay impls are deleted. The transcript viewer's help holds a `Pager<HelpPage>` until CSP-748.

Verified: tuikit unit, doc, independence, and documentation tests; host value modal, keymap help, viewer help, help parity, scrolling, conventions, app, and snapshot tests. The full isolated suite passed (4357 passed, 0 failed, 1 ignored), and nextest passed. Also passed: fmt, clippy -D warnings, rustdoc -D warnings, `cargo build --examples`, and `git diff --check`. I compared `--snapshot` frames before and after at 100x30, 60x20, and 40x10 (in /tmp/tuifw-baseline/frames-746-{before,after}): every keymap help frame is byte-identical.

**Regressions**
- An empty value now says `nothing to show` instead of showing a blank body (the explorer never opens one).
- When the value overflows, its text wraps one cell narrower beside the scrollbar.

**Consistency wins**
- The value modal footer puts the keys that leave last, as every pager's footer does: `j/k/PgDn/PgUp scroll · y copy · Esc/q/o close`.
- The value modal footer is drawn in the help page's dim look instead of the inert color.
- On a narrow screen the footer wraps onto a second row instead of being cut (at 40 columns `y copy` was lost).
- A long value shows the shared scrollbar.
- `G` keeps to the last page, and paging moves by the rows on screen, in both modals.
<!-- SECTION:FINAL_SUMMARY:END -->
