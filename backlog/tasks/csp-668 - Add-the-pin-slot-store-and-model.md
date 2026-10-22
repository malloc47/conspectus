---
id: CSP-668
title: Add the pin slot store and model
status: To Do
assignee: []
created_date: '2026-10-22 01:11'
labels:
  - history-pins
milestone: m-23
dependencies:
  - CSP-657
  - CSP-667
documentation:
  - docs/plans/history-rebuild-and-pin-slots.md
priority: high
type: feature
ordinal: 726000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Pins become editable slots that can point at any rebuildable entity, live or gone, kept in a user-scope store and ordered. This replaces `[[pins.entries]]` in project and user config, the pin-store registry, and the pin binding pass.

- Scope: `$XDG_DATA_HOME/conspectus/pins.toml` with `[pins] schema_version = 2` and `[[pins.slots]]` (id, kind, target, overrides, snapshot, `pinned_at`, `order`), written with `toml_edit` round-trip; pinnable kinds per the ADR, runtime processes rejected; effective recipe resolved per field (overrides > live graph > history > snapshot > defaults); session slots resolve to the lineage head; planned slots (no target); dangling targets render from the snapshot; new slots append to the order; history records targeted by a slot are protected from reaping; `PinNode` reworked with kind, target, overrides, and effective recipe; `pin_targets` to any node, gone included; the binding pass, `pin_realized_by_session`, and `Pin*` diagnostics removed; the daemon never writes `pins.toml`.
- Tests: store round-trip and ordering tests; precedence tests per field and kind; lineage, planned, and dangling cases; a test that a daemon cycle leaves `pins.toml` untouched.
- Manual checks: pin a live mux, kill it, and confirm the slot shows the ghost with its overrides applied.
- Blockers: the pin slots ADR and the graph projection ticket.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 Slots of every pinnable kind persist in `pins.toml` with their order, and operator comments survive a write
- [ ] #2 Each effective recipe field comes from the highest-precedence source that has it, with tests per kind
- [ ] #3 A slot can be planned (no target), dangling, or pointed at a gone entity, and renders in each case
- [ ] #4 The pin binding pass and the `Pin*` diagnostics are gone, and `PinNode` connects to its target through `pin_targets`
- [ ] #5 Background processes never write `pins.toml`
<!-- AC:END -->

## Definition of Done
<!-- DOD:BEGIN -->
- [ ] #1 `just check` passes, or `git diff --check` for docs-only changes
- [ ] #2 Docs and ADRs are updated when behavior, architecture, or workflow change
<!-- DOD:END -->
