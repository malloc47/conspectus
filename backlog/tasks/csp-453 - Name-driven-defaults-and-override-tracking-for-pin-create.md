---
id: CSP-453
title: Name-driven defaults and override tracking for pin create
status: Done
assignee: []
created_date: '2026-06-24 20:11'
labels:
  - h-pin-tui
milestone: m-11
dependencies:
  - CSP-452
ordinal: 319000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
- Scope: in the create form, seed a single primary name from the
  selected row and derive the persisted pin id, display name, and
  mux name from it until the operator edits one of those fields
  directly. Preserve explicit overrides after later name edits.
  Defaults should distinguish "pin/adopt this selected live entry"
  from "create a fresh pin": exact adoption should preserve
  the selected mux name, while fresh-session creation should propose
  a non-conflicting mux name derived from the selected cwd/workspace
  and name. Keep the underlying TOML schema unchanged.
- Tests: reducer tests for selected session, selected mux, selected
  repo/checkout/workspace, blank launch, name edits before and
  after an explicit field override, duplicate-id / duplicate-mux
  preflight, and static-scenario no-write behavior.
- Blockers: `CSP-452`.
<!-- SECTION:DESCRIPTION:END -->

## Final Summary

<!-- SECTION:FINAL_SUMMARY:BEGIN -->
The create form now starts with `name`, derives
persisted id, display name, and mux name from it, and preserves
explicit advanced-field overrides after later name edits. Clearing
a derived advanced field returns it to automatic derivation on the
next name edit. The Pins menu exposes one `create` flow; when the
selected row is adoptable, the create form opens with `adopt
selected` checked and the selected mux's original name populated.
The first operator edit to the primary name automatically unchecks
adopt and turns the form into `new` mode; if the operator
manually checks adopt again, later name edits keep it checked. In
that opt-in state, the editable `mux.name` stays aligned with the
new pin name and previews the operation as
`<new-name> (rename of: <old-name>)`; on commit the TUI asks tmux
to rename the adopted mux to that target. If the effective
`mux.name` exactly matches a known live mux, whether derived from
the primary name or directly edited, the form automatically
re-checks adopt to avoid creating a pin that shadows a tmux
session name; when that automatic collision check no longer
applies, the form automatically returns to `new` mode unless the
operator manually intervened. The default `new` alternative still
proposes a non-conflicting mux name when the selected context's
mux name is already live or pinned.
`A` remains a direct accelerator into the same create form with
adopt selected, not a separate menu-level command.
<!-- SECTION:FINAL_SUMMARY:END -->

Legacy ID: `H-PIN-TUI-002`
