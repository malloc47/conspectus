---
id: CSP-690
title: Retire the legacy pin loader
status: To Do
assignee: []
created_date: '2026-10-22 01:11'
labels:
  - history-pins
milestone: m-23
dependencies:
  - CSP-671
  - CSP-689
documentation:
  - docs/plans/history-rebuild-and-pin-slots.md
priority: low
type: chore
ordinal: 748000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Legacy `[[pins.entries]]` loading, the ADR 0058 pin-binding sidecar, and the ADR 0090 pin-store registry stay for one release so upgraders can migrate. After that they are dead code.

- Scope: remove the legacy loader and `LegacyPins` diagnostic, the pin-bindings cache directory handling, and `pin-stores.json` reading; leave a short note in `pin migrate`'s replacement message, or remove the command.
- Tests: existing tests updated; no legacy fixtures remain.
- Manual checks: none.
- Blockers: the migration ticket and the 0.2.0 release.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 No code reads `[[pins.entries]]`, the pin-bindings directory, or `pin-stores.json`
<!-- AC:END -->

## Definition of Done
<!-- DOD:BEGIN -->
- [ ] #1 `just check` passes, or `git diff --check` for docs-only changes
- [ ] #2 Docs and ADRs are updated when behavior, architecture, or workflow change
<!-- DOD:END -->
