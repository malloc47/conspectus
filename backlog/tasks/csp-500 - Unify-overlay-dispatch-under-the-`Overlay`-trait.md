---
id: CSP-500
title: Unify overlay dispatch under the `Overlay` trait
status: Done
assignee: []
created_date: '2026-07-02 21:01'
labels:
  - h-tui
milestone: m-11
dependencies: []
ordinal: 109000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
- Landed 2026-07-03. Chose the "grow the trait" direction:
  `Overlay` gains a GAT `type Ctx<'a>` so context-free
  widgets (Help / ValueModal / Viewer / Rename) declare
  `type Ctx<'a> = ()` and contextual widgets (Controls / Pins
  / Search) declare a lifetime-carrying reference type. Also
  grew `OverlayOutcome` with `CommitAndStay(Box<Msg>)` so
  Controls (`ApplyAndStay`, toggle chip in place) and Pins
  (`ApplyAndStay`, arm Delete confirmation) map cleanly.
  Rename gets a new `RenameOverlayState` wrapper around
  `TextInputState` that maps `Confirm(String)` to
  `Commit(Box::new(Msg::CommitRename(text)))`. Search's
  Confirm(RowId) flows through a new `Msg::SelectRow(Box<RowId>)`
  reducer arm instead of a direct `app.set_selection` call.
  All seven Modal variants now implement `Overlay`; the four
  specialized `handle_*_overlay_key` runtime helpers still
  exist as thin dispatch shims (each just calls
  `state.handle(ctx, key)` and matches on the four-variant
  OverlayOutcome). modal.rs's per-variant doc comments no
  longer list "doesn't implement Overlay yet."
<!-- SECTION:DESCRIPTION:END -->

Legacy ID: `H-TUI-006`
