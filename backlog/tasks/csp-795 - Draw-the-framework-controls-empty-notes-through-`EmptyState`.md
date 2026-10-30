---
id: CSP-795
title: Draw the framework controls' empty notes through `EmptyState`
status: To Do
assignee: []
created_date: '2026-10-30 14:19'
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
- [ ] #1 LogList, Palette, and Pager draw their empty-list text through EmptyState, and a host can give each a hint
- [ ] #2 Their default and host-supplied copy is unchanged
- [ ] #3 Every visible change is listed in the final summary under Regressions and Consistency wins, and was checked with conspectus tui --snapshot at a normal and a narrow width
- [ ] #4 The control pages of the three controls and of EmptyState say so, and the independence guard passes
<!-- AC:END -->

## Definition of Done
<!-- DOD:BEGIN -->
- [ ] #1 `just check` passes, or `git diff --check` for docs-only changes
- [ ] #2 Docs and ADRs are updated when behavior, architecture, or workflow change
<!-- DOD:END -->
