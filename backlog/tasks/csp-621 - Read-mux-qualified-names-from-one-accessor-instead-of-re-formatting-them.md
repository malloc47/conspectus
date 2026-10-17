---
id: CSP-621
title: Read mux qualified names from one accessor instead of re-formatting them
status: To Do
assignee: []
created_date: '2026-10-17 03:09'
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
- [ ] #1 No production code rebuilds a mux qualified name with an inline `format!` over `backend` and `native_id`
- [ ] #2 The pin-binding lookup key and the displayed mux label each come from a named accessor
- [ ] #3 Output and TUI snapshots are unchanged
<!-- AC:END -->

## Definition of Done
<!-- DOD:BEGIN -->
- [ ] #1 `just check` passes, or `git diff --check` for docs-only changes
- [ ] #2 Docs and ADRs are updated when behavior, architecture, or workflow change
<!-- DOD:END -->
