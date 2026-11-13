---
id: CSP-795
title: Draw the framework controls' empty notes through `EmptyState`
status: Done
assignee:
  - '@claude'
created_date: '2026-10-30 14:19'
updated_date: '2026-10-30 20:59'
labels:
  - tui-framework
milestone: m-25
dependencies:
  - CSP-758
references:
  - >-
    docs/adr/0190-tui-the-chrome-gives-way-by-one-rule-and-an-empty-slot-says-so-one-way.md
priority: low
type: enhancement
ordinal: 881000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
CSP-758 added `tuikit::empty_state::EmptyState` and moved the host's pane and Preview empty states and `TreeView`'s placeholder onto it. Three framework controls still draw what their empty list says with code of their own: `LogList` (`empty`, default `nothing logged yet`, drawn in `log_list/body.rs`), `Palette` (`empty_note` and the no-match note, `(nothing to pick)` and `(no matches)`), and `Pager` (`PagerContent::empty`, `nothing to show`, wrapped in `pager/body.rs`). Each wraps and mutes on its own, so the three can drift from the one look, and none takes a hint. The host's copy for them (the Messages overlay's, the pickers' and quick select's notes) stays the host's.

Out of scope: the empty Clean up dialog notice (a `Dialog` notice by design) and a minimum-size screen (CSP-319, CSP-190).
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [x] #1 LogList, Palette, and Pager draw their empty-list text through EmptyState, and a host can give each a hint
- [x] #2 Their default and host-supplied copy is unchanged
- [x] #3 Every visible change is listed in the final summary under Regressions and Consistency wins, and was checked with conspectus tui --snapshot at a normal and a narrow width
- [x] #4 The control pages of the three controls and of EmptyState say so, and the independence guard passes
<!-- AC:END -->

## Definition of Done
<!-- DOD:BEGIN -->
- [x] #1 `just check` passes, or `git diff --check` for docs-only changes
- [x] #2 Docs and ADRs are updated when behavior, architecture, or workflow change
<!-- DOD:END -->

## Implementation Plan

<!-- SECTION:PLAN:BEGIN -->
1. `LogList` keeps an `EmptyState` (`.empty(impl Into<EmptyState>)`, default `nothing logged yet`) and draws it with `EmptyState::lines` over the footer's spilled rows.
2. `Palette` keeps two `EmptyState`s (`empty_note`, `no_match_note`, and the `set_` forms take `impl Into<EmptyState>`); the body draws the note's wrapped lines and counts them in `list_rows(width)` for its measure and footer fit.
3. `PagerContent::empty` returns an `EmptyState` (default `nothing to show`); the body draws its lines before the tail. The host's two contents and the test content return theirs.
4. Tests: a hint and a wrap for each control, in the muted look; the defaults and the host strings unchanged.
5. Docs: the empty state page (which controls take one), the log list, palette, and pager pages; ADR 0190 decision 7 note, design.md, CHANGELOG, operator review.
6. Verify with `conspectus tui --snapshot` of every framed modal and the empty notes at 100x30, 60x20, 40x10, and narrower, before and after.
<!-- SECTION:PLAN:END -->

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
Implemented the three controls on EmptyState; 13 of 156 snapshot frames differ (the empty Messages note now muted; content search's note wrapping at 30 columns). Full suite 4761 passed after one rerun for the known discovery::forge spawn flake.
<!-- SECTION:NOTES:END -->

## Final Summary

<!-- SECTION:FINAL_SUMMARY:BEGIN -->
LogList, Palette, and Pager draw their empty-list text through `EmptyState`, and a host can give each a hint (commits 1a4c8f91, 3442c480). `LogList::empty`, `Palette::empty_note`/`no_match_note` and `set_empty_note`/`set_no_match_note` take `impl Into<EmptyState>`; `PagerContent::empty` returns an `EmptyState` (it returned `Cow<str>`; the pane scrollback and transcript contents now return theirs). Each draws `EmptyState::lines` at its slot's width before what else the slot holds (the spilled footer rows, the pager's tail); the palette counts the note's wrapped rows for its measure and footer fit. Defaults (`nothing logged yet`, `(nothing to pick)`, `(no matches)`, `nothing to show`) and the host's copy (the Messages note with its line break, the pickers' and searches' notes, `the pane has no scrollback`, `transcript unavailable`, `every turn here is hidden`) are unchanged. Docs: the empty state page (a table of the controls that take one), the log list, palette, and pager pages, ADR 0190 (decision 7 extended), design.md, CHANGELOG, operator review.

Verification: framework tests give each control a hint and check its rows and the muted look (a log list's hint wrapping at 40 columns, a palette's message and hint on their own rows, a pager content's hint), and a palette note wider than its list wraps whole with no `…`; the existing default and host-copy tests pass unchanged. `conspectus tui --snapshot` before/after of every framed modal at 100x30, 60x20, and 40x10, nine empty-note states (quick select and content search with no match in both scopes, the empty Messages log, the Search view, the viewers) at those sizes, and content search and Messages at 30, 34, and 36 columns by 14 rows: 13 of 156 frames differ (Frames in /tmp/tuifw-baseline/frames-795-{before,after}). fmt, clippy `-D warnings`, the independence guard, the isolated suite (4761 passed, 0 failed, 1 ignored; one rerun after the known `discovery::forge` spawn flake), rustdoc `-D warnings`, `cargo build --examples`, and `git diff --check` pass.

**Regressions**
- On a palette too short for a wrapped note's rows, only the first rows show and no `…` marks the rest: content search at 30x14 shows `(type to search what was` where it showed `(type to search what was sa…`. At 34 columns and wider (and at every size the suite checks, 40x10 up) the whole note shows.

**Consistency wins**
- The empty Messages log's note (`No messages yet. Launches, attaches, renames and` / `failures from this session are listed here.`) recedes in the `muted` look every other empty slot has, at 100x30, 60x20, and 40x10; its words and rows are unchanged.
- A palette's empty or no-match note wraps at word edges instead of being cut with `…` (no change at 40 columns and wider with the host's notes).
- An empty pager's note is the shared empty state (same words, look, and wrap as before).
<!-- SECTION:FINAL_SUMMARY:END -->
