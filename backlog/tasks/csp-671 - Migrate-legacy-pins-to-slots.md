---
id: CSP-671
title: Migrate legacy pins to slots
status: To Do
assignee: []
created_date: '2026-10-22 01:11'
labels:
  - history-pins
milestone: m-23
dependencies:
  - CSP-668
  - CSP-664
documentation:
  - docs/plans/history-rebuild-and-pin-slots.md
  - docs/adr/0090-pin-store-registry-sidecar.md
priority: high
type: feature
ordinal: 729000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Existing `[[pins.entries]]` (15 in the operator's setup, most with custom launch argv, two running non-agent programs, some project-local) map one-to-one onto mux slots, so migration can be lossless. Until it runs, nothing should disappear on upgrade.

- Scope: `conspectus pin migrate [--dry-run]`: read project stores (through the ADR 0090 registry) and the user store; write one mux slot per entry keeping its `id`, with overrides for `mux.name`, `socket_name`, `cwd`, `harness`, `launch.argv`, and `worktree.branch` where they differ from defaults; seed a history record with `last_session` from the ADR 0058 sidecar and target the slot at it; write an explicit alias where a pin's `display_name` differs from both its mux name and its bound session's title; remove converted tables keeping comments and formatting; print a report. Until migrated, legacy entries load as in-memory slots and a `LegacyPins` diagnostic suggests the command.
- Tests: migration tests over today's pin shapes (custom argv, non-agent harness, non-default socket, worktree-backed, never launched), dry run, idempotent rerun, comment preservation.
- Manual checks: migrate a copy of the real config and compare `pin list` before and after.
- Blockers: the slot store and mux recording tickets.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 Every legacy pin becomes a slot with the same id and equivalent effective recipe, and keeps its resumable last session
- [ ] #2 `--dry-run` changes nothing, and a second run is a no-op
- [ ] #3 Before migration, legacy pins still appear as slots, and a diagnostic names the command
<!-- AC:END -->

## Definition of Done
<!-- DOD:BEGIN -->
- [ ] #1 `just check` passes, or `git diff --check` for docs-only changes
- [ ] #2 Docs and ADRs are updated when behavior, architecture, or workflow change
<!-- DOD:END -->
