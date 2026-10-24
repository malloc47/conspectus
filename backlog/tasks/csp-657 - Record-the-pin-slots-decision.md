---
id: CSP-657
title: Record the pin slots decision
status: Done
assignee:
  - '@claude'
created_date: '2026-10-22 01:10'
updated_date: '2026-10-22 02:34'
labels:
  - history-pins
milestone: m-23
dependencies:
  - CSP-656
documentation:
  - docs/plans/history-rebuild-and-pin-slots.md
  - docs/adr/0084-first-class-pin-nodes.md
  - docs/adr/0090-pin-store-registry-sidecar.md
priority: high
type: docs
ordinal: 715000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
With history recording everything rebuildable, a pin no longer has to carry a mux's lifecycle. The operator still wants pins to be editable slots, as they are today (rename, change the launch command, point at something else), and wants to pin anything that could be rebuilt, not only muxes. This ADR records the generalized pin model from the plan.

- Scope: slot shape (target, overrides, snapshot, `pinned_at`, `order`); per-field precedence overrides > live graph > history > snapshot > defaults, with snapshots written only by operator gestures; editing gestures (pin, edit, rename with lockstep, retarget, refresh snapshot, unpin); overridable fields per kind (mux: name, socket, cwd, harness, argv, worktree branch); planned slots with no target (kept, per review); ordering stored in the slot store with new slots appended; pinnable kinds (everything except runtime processes); session slots following lineage; reap protection; storage in `$XDG_DATA_HOME/conspectus/pins.toml`, user scope only, schema v2; `PinNode` kept as the slot's graph identity with a generic `pin_targets` relation; removal of the binding pass, `pin_realized_by_session`, and the `Pin*` diagnostics; native views stop floating pinned items.
- Tests: docs-only; `git diff --check`.
- Manual checks: none.
- Blockers: the history ADR.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [x] #1 A new ADR records the slot model, precedence, editing gestures, overridable fields per kind, planned slots, ordering, pinnable kinds, storage location, and graph projection
- [x] #2 The ADR supersedes ADR 0057's schema, CLI, and TUI sections and ADR 0090, and amends ADR 0084; status lines on those ADRs point at it
- [x] #3 Alternatives considered include pins as pure references, project-local pin stores, `config.toml` as the store, and a daemon-refreshed snapshot
<!-- AC:END -->

## Definition of Done
<!-- DOD:BEGIN -->
- [x] #1 `just check` passes, or `git diff --check` for docs-only changes
- [x] #2 Docs and ADRs are updated when behavior, architecture, or workflow change
<!-- DOD:END -->

## Implementation Plan

<!-- SECTION:PLAN:BEGIN -->
1. Write ADR 0141 from the plan's Pin slots sections.
2. Index it and put status lines on ADRs 0057, 0084, and 0090.
<!-- SECTION:PLAN:END -->

## Final Summary

<!-- SECTION:FINAL_SUMMARY:BEGIN -->
Recorded ADR 0141 (pin slots): slot shape (target, overrides, snapshot), field precedence, gestures, overridable fields per kind, planned slots, table order as slot order, pinnable kinds, `$XDG_DATA_HOME/conspectus/pins.toml` with schema v2, `PinNode` with `pin_targets`, the legacy loader and `pin migrate`, and the CLI. Supersedes ADR 0057's schema, CLI, and TUI and ADR 0090; amends 0084. Alternatives cover pure references, project-local stores, `config.toml`, a daemon-refreshed snapshot, and explicit order numbers. Verified with `git diff --check`.
<!-- SECTION:FINAL_SUMMARY:END -->
