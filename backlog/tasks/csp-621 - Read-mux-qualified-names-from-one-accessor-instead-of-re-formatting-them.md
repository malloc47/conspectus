---
id: CSP-621
title: Read mux qualified names from one accessor instead of re-formatting them
status: Done
assignee:
  - '@claude'
created_date: '2026-10-17 03:09'
updated_date: '2026-10-18 00:11'
labels:
  - h-hyg
milestone: m-11
dependencies: []
references:
  - docs/code-hygiene-audit.md
priority: medium
type: chore
ordinal: 679000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Every mux backend builds a node id of the form `<backend>:<native_id>` (`src/discovery/tmux/mod.rs:1558-1560`, `src/discovery/zellij/mod.rs:260-262`), yet 12 production sites in 9 files rebuild that string inline with `format!("{}:{}", backend, native_id)`: `output/agent.rs`, `output/mux.rs`, `output/union.rs`, `output/node_show.rs` (2), `resolve/pins.rs:415` (as the pin-binding lookup key), `tui/actions.rs`, `tui/detail.rs` (2), `tui/explorer.rs` (2), and `tui/runtime/launch.rs`. Each copy assumes that format and which field holds which half. CSP-617 is that drift already happening: `pin adopt` put the prefixed form on the wrong field. CSP-188's display-label work also needs one source for the mux label. Audit finding A8, re-scoped: the per-projection `cell` dispatchers share almost no column keys, but they and the TUI repeat these value formatters.

- Scope: add named accessors on `MuxSessionNode` (or use `id` directly where the string is the id) and route the 12 sites through them. Keep identity uses (the pin lookup key) separate from display uses so CSP-188 can change the label without touching identity.
- Tests: output, pin-resolver, and TUI snapshot suites pass unchanged; a model test pins the accessors' output for tmux default and named sockets and for zellij.
- Blockers: none. CSP-617 can land first as the targeted fix.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [x] #1 No production code rebuilds a mux qualified name with an inline `format!` over `backend` and `native_id`
- [x] #2 The pin-binding lookup key and the displayed mux label each come from a named accessor
- [x] #3 Output and TUI snapshots are unchanged
<!-- AC:END -->

## Definition of Done
<!-- DOD:BEGIN -->
- [x] #1 `just check` passes, or `git diff --check` for docs-only changes
- [x] #2 Docs and ADRs are updated when behavior, architecture, or workflow change
<!-- DOD:END -->

## Implementation Plan

<!-- SECTION:PLAN:BEGIN -->
1. `MuxSessionNode::qualified_name()` is the identity form `<backend>:<native_id>` (what a pin's `mux.native_id()` and an operator-typed `node show` reference name); `MuxSessionNode::display_label()` is the operator-facing label, today the same string, so CSP-188 can change it alone.
2. Identity sites use `qualified_name()`: the pin-binding key in `resolve/pins.rs` (replacing `pin_key_for_mux`) and the operator-input match in `output/node_show.rs`.
3. Display sites use `display_label()`: `output/{mux,union,node_show}.rs`, `tui/detail.rs` (drops `mux_display_label`), `tui/explorer.rs` (two label functions).
4. `output/agent.rs::MuxInfo` and `output/union.rs::PreferredMux` copied `backend` and `native_id` only to format them; they carry the computed label instead.
5. `AttachTarget::label()` replaces the duplicate `tui::actions::target_label` and `tui::runtime::launch::target_short`.
6. `PinLaunchTarget` holds the pin's `PinMuxRef`, so the attach id comes from `PinMuxRef::native_id()` instead of a hand-built string (and keeps the socket for socketed pins).
7. Tests: model test pinning both accessors for tmux default socket and zellij; `AttachTarget::label` test; output and TUI snapshot suites unchanged.
<!-- SECTION:PLAN:END -->

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
- `MuxSessionNode::qualified_name()` (identity: pin-binding key in `resolve/pins.rs`, operator-input match in `output/node_show.rs`) and `MuxSessionNode::display_label()` (tables, detail, explorer) replace 11 inline formats over `backend`/`native_id`. `tui/detail.rs::mux_display_label` is gone.
- `output/agent.rs::MuxInfo` and `output/union.rs::PreferredMux` carried `backend` and `native_id` only to format them; they now carry `label`. The union relationship cell used a multi-line `"mux={}:{} [{}]"` format the ticket's single-line count missed.
- `AttachTarget::label()` replaces the identical `tui::actions::target_label` and `tui::runtime::launch::target_short`.
- `PinLaunchTarget` holds the pin's `PinMuxRef`, so the TUI attach id is `PinMuxRef::native_id()` (socket-qualified for socketed pins) instead of a hand-built string.
- Validation: a multi-line regex scan of production code finds `backend:native_id` formatting only inside the two accessors; new tests `model::tests::mux_qualified_name_is_backend_and_native_id_and_matches_the_pin_key` and `tui::actions::tests::attach_target_label_is_backend_and_native_name`; `just check` green (2400 tests) with every output and TUI snapshot unchanged.
<!-- SECTION:NOTES:END -->

## Final Summary

<!-- SECTION:FINAL_SUMMARY:BEGIN -->
Mux qualified names now come from named accessors instead of 14 inline `format!` calls: `MuxSessionNode::qualified_name()` for identity (pin binding, `node show` lookup), `MuxSessionNode::display_label()` for what operators see, and `AttachTarget::label()`, which also replaces two duplicate helper functions. The pin launch target carries the pin's `PinMuxRef` so its attach id comes from `PinMuxRef::native_id()`. Output is unchanged: `just check` passes (2400 tests) with all snapshots identical.
<!-- SECTION:FINAL_SUMMARY:END -->
