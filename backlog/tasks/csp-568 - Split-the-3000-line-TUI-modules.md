---
id: CSP-568
title: 'Split the 3,000-line TUI modules'
status: Done
assignee: []
created_date: '2026-10-01 00:23'
labels:
  - h-rust
milestone: m-19
dependencies: []
ordinal: 577000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
- Problem: `tui/app.rs` (3,581 lines, with a 420-line `App::update`
  `match`), `tui/runtime.rs` (3,483), `tui/ui.rs` (3,384), and
  `tui/widgets/pins.rs` (3,193) are hard to navigate.
- Plan: split the reducer into per-family handlers (navigation,
  overlays, pins, worktrees, mux), the runtime into per-executor
  modules, `ui.rs` by pane, and `pins.rs` by form. Moves only, no
  behavior change; the existing snapshot tests are the safety net.
<!-- SECTION:DESCRIPTION:END -->

## Final Summary

<!-- SECTION:FINAL_SUMMARY:BEGIN -->
Moves only, guarded by the render snapshot tests.
`tui/app.rs` 3,566 → 1,522 lines (`app/{msg,overlays,pins,tree,
explorer_nav}.rs`); `tui/runtime.rs` 3,482 → 1,574
(`runtime/{executor,worktree_exec,pin_store,launch,overlay_keys}.rs`);
`tui/ui.rs` 3,374 → 314 (`ui/{header,left_panel,right_panel,
detail_header,preview_pane,text}.rs`); `tui/widgets/pins.rs` 3,193 →
745 (`pins/{create,edit,bind,create_widget,widgets}.rs`). The reducer
stays one flat `match` in `App::update`: its arms are short, so
splitting it per family would only add indirection. The one long arm,
`CommitRename`, became `App::rename_commit_effect`.
<!-- SECTION:FINAL_SUMMARY:END -->

Legacy ID: `H-RUST-015`
