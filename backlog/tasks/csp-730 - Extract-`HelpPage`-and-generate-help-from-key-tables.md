---
id: CSP-730
title: Extract `HelpPage` and generate help from key tables
status: Done
assignee:
  - '@claude'
created_date: '2026-10-27 20:26'
updated_date: '2026-10-28 23:30'
labels:
  - tui-framework
  - tuifw-conventions
milestone: m-25
dependencies:
  - CSP-728
  - CSP-723
  - CSP-724
  - CSP-726
documentation:
  - docs/plans/tui-framework/inventory/05-search-help-viewers.md
  - docs/plans/tui-framework/inventory/06-interaction-conventions.md
priority: medium
type: enhancement
ordinal: 816000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Three help generators exist (`viewer_help.rs:39`, `pane_viewer.rs:563`, and the keymap help), and the pane viewer's page is a copy of the transcript viewer's. The main help is 48 hand-written rows with a one-directional drift test, and its blurb cites Ctrl-L where the modal binds S-Tab. `ratatui-cheese`'s `help::Binding` is used only as an inert struct. The page itself pages by a fixed 8 lines and never clamps its over-scroll (inventory slices 05 and 06).

Scope: Create `tuikit::help` with a page renderer (frame, section headings, key column in the accent colour, wrapped descriptions, scrollbar, generated footer) and a `sections_for<A>` generator over a key table plus per-action descriptions. Delete the pane help copy and `ratatui-cheese::help::Binding`. The main keymap page keeps its hand-written rows as annotations until the scoped keymap story generates them. Page by the viewport and clamp the over-scroll.

TUI changes:
- Consistency wins: three help pages share one generator and one frame; help pages can no longer disagree with their key tables
- Regressions or behavior changes: none expected

Tests and checks: Content and styling parity tests (extend the CSP-703 test) for the three pages. `--snapshot` of each help page at 80 and 40 columns.

Blockers: The key-table, text, scroll, and frame extractions.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [x] #1 `tuikit::help` provides the page renderer and a generator from a key table and its descriptions
- [x] #2 The transcript viewer and pane viewer pages come from the generator and the pane copy is deleted
- [x] #3 The `ratatui-cheese` `help::Binding` use is removed
- [x] #4 The page pages by the viewport and clamps its over-scroll
- [x] #5 A test fails when a key in a table has no description in its generated page
- [x] #6 Every visible change is listed in the final summary under **Regressions** and **Consistency wins**, and was checked with `conspectus tui --snapshot` at a normal and a narrow width
- [x] #7 The framework crate names no Conspectus type, string, theme field, or domain word, and the independence guard passes
- [x] #8 The control has a reference page in the framework docs: purpose, usage, key table, theme tokens used, and the house rules it follows
<!-- AC:END -->

## Definition of Done
<!-- DOD:BEGIN -->
- [x] #1 `just check` passes, or `git diff --check` for docs-only changes
- [x] #2 Docs and ADRs are updated when behavior, architecture, or workflow change
<!-- DOD:END -->

## Implementation Plan

<!-- SECTION:PLAN:BEGIN -->
1. Snapshot mode: open the pane viewer from a fixture world's recorded pane captures, so the pane help page can be captured; capture before frames of the three help pages (page 1, paged, end, over-scrolled) at 100x30, 60x20, 80x24, 40x12.
2. `tuikit::help`: content model (`HelpSection`, `HelpRow` keys and legend rows, `HelpEntry`), the `HelpAction` trait (listed order + section and description per action), the `sections_for<A>` generator over a key table (one row per action, `key_label` keys, plain before chords, joined ` / `, grouped by section, unbound actions left out, unavailable rows dimmed), the `undescribed` check; `HelpPage` renderer (ModalFrame at WIDE, `Help` title, bold headings, accent key column, descriptions wrapped with `tuikit::text`, legends, scrollbar via `ScrollGutter`, footer generated from a key table and fixed at the bottom); `HelpScroll` (steps may ask past the end, the draw's window takes them back, page keys move by the rows shown). Unit tests and docs/controls/help.md; architecture map flipped.
3. Host: the main keymap help keeps its hand-written rows and four legends as annotation sections drawn through `HelpPage`; the transcript and pane viewers' pages come from `sections_for` through one adapter; delete the pane viewer's copy, the viewer_help generator, the old page renderer and row helpers, and the `ratatui-cheese` `help::Binding` use (the checklist shim keeps the crate until CSP-741). Viewer and pane viewer state keep a `HelpScroll`.
4. Tests: every bound key of the viewer and pane tables is described on its page (fails otherwise); the CSP-703 parity test covers all three pages (frame, key and description columns, headings, footer, scrolling); port the existing help tests.
5. ADR for the help page and generator, index, operator review; after frames, compare, list Regressions and Consistency wins; full checks.
<!-- SECTION:PLAN:END -->

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
Landed in five commits: snapshot mode opens the pane viewer from a fixture world (9496e9d); the `tuikit::help` module with its reference page (dd9299c); the host on it, with the pane and viewer generators, the old renderer, and `help::Binding` deleted (40d46a4); the content search row's `S-Tab` correction with a test against the search's table (d097b9a); ADR 0162, design notes, changelog, and operator review (68cd008).

Verification: isolated full suite 3875 passed, 0 failed, 1 ignored (no flakes, no reruns); fmt check, clippy with -D warnings over the workspace, rustdoc with -D warnings, the whitespace diff check, and the tuikit guards (independence, documentation, raw styles, glyph literals) pass. Before/after `--snapshot` frames of the three help pages (page 1, j x3, one and two pages down, end, end then k, over-scrolled, end then PgUp; viewer and pane pages paged, half-paged, at the end, over-scrolled) at 100x30, 60x20, 80x24, and 40x12 are in /tmp/tuifw-baseline/frames-730-{before,after}: 79 of 80 byte-identical, the 80th only the `S-Tab` wording. A probe at 40x12 shows the explorer heading wrapping where it was clipped. The reference page's example was compiled and run once as a scratch test.

`Cargo.toml` is unchanged: `ratatui-cheese` has no features and the checklist shim still uses it until CSP-741.
<!-- SECTION:NOTES:END -->

## Final Summary

<!-- SECTION:FINAL_SUMMARY:BEGIN -->
`tuikit::help` now owns the one help page and its generator. `HelpPage` draws in `ModalFrame` at `ModalSize::WIDE`, titled `Help`: bold word-wrapped headings, keys in `accent` in a 16-cell column with descriptions wrapped beside them through `tuikit::text`, legend rows (a host-styled mark, a bold name, a blurb), the `ScrollGutter` scrollbar when the page overflows, and a muted footer generated from the key table passed to `footer(table)` that stays on the last rows. `sections_for<A: HelpAction>(table)` makes one row per listed action the table binds (every key, `key_label`, plain before Ctrl/Alt chords, joined ` / `), grouped by the section each action's `HelpEntry` names, dimmed when no key is available; `undescribed(table)` returns the keys a page would leave out. `HelpScroll` keeps the offset asked for, `keep(window)` takes it back inside the page after each draw, and page keys move by the rows shown (half pages by half). ADR 0162; reference page `crates/tuikit/docs/controls/help.md`.

Host: the keymap help keeps its 48 hand-written rows and four legends as annotation sections drawn by `HelpPage`; `help::page` gives all three pages the `HELP_KEYS` footer. `ViewerMsg` and `PaneViewerMsg` implement `HelpAction`, and both viewers draw their generated page through one `viewer_help::draw(table, scroll, ..)`. Deleted: the pane viewer's generator copy and row type, the viewer generator and row types, the old renderer and its row helpers, the three scroll arithmetics, and the `ratatui_cheese::help::Binding` rows (`Cargo.toml` unchanged: the crate has no features and the checklist shim keeps it until CSP-741). Snapshot mode now opens the pane viewer from a fixture world's recorded captures, so its help can be captured (`srelease summary<Enter>?`).

**Regressions**
- None.

**Consistency wins**
- The three help pages share one generator, one page, one frame, one column grid, one heading style, and one footer; the pane viewer's page is no longer a copy, and a key in either viewer's table cannot be left off its page (a test fails).
- The keymap help's content search row says `S-Tab switches between listing sessions and listing messages` (it said `Ctrl-L`, which is the Search view's key).
- A heading too long for a narrow page wraps instead of being clipped by the frame (at 40 columns `Detail-pane graph explorer (right focus)` takes two rows).
- Everything else is unchanged: 80 states (the keymap help at page 1, paged, at the end, over-scrolled and back; the viewer and pane pages paged, half-paged, at the end, over-scrolled) at 100x30, 60x20, 80x24, and 40x12 are byte-identical ANSI, except the one frame showing the `S-Tab` wording.

Tests: framework unit and property tests for the generator, layout, footer, and scrolling; host tests that every key either viewer binds is described once on its page and that an undescribed key fails; the CSP-703 parity test now covers all three pages (frame, key and description columns, headings, footer row, scrolling); the old help tests ported. Verified with the isolated full suite (3875 passed, 0 failed, 1 ignored), fmt, clippy -D warnings, rustdoc -D warnings, the whitespace diff check, and the independence guard.
<!-- SECTION:FINAL_SUMMARY:END -->
