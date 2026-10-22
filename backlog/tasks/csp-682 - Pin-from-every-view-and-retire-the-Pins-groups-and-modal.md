---
id: CSP-682
title: Pin from every view and retire the Pins groups and modal
status: To Do
assignee: []
created_date: '2026-10-22 01:11'
labels:
  - history-pins
milestone: m-23
dependencies:
  - CSP-679
  - CSP-670
documentation:
  - docs/plans/history-rebuild-and-pin-slots.md
  - docs/adr/0097-launch-spec-form-primitive.md
priority: medium
type: feature
ordinal: 740000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Pinning becomes a gesture available on any pinnable row, and slots are edited in place. The old pin surfaces (Pins groups, placeholder rows, the Pins modal, and the `N` / `B` / `b` / `A` keys) go away. This makes the Pins half of CSP-619 moot.

- Scope: pin and unpin as row actions in every view, menu-first with one accelerator; a one-cell pin marker and a `pinned only` Controls filter in native views; the slot edit form (reusing the launch-spec form, ADR 0097) and a retarget picker; removal of the Pins groups, placeholder session and mux rows, the Pins modal, and the old keys; `N` opens mux launch with the worktree toggle; the mux launch form gains "pin" and "pin, don't launch".
- Tests: action and keymap tests; snapshot tests of native views with pinned rows.
- Manual checks: pin, edit, retarget, and unpin from each view with `--snapshot-keys`.
- Blockers: the Pinned view and pin CLI tickets.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 Any pinnable row in any view can be pinned or unpinned from its action menu
- [ ] #2 Native views mark pinned rows without floating them and can filter to pinned only
- [ ] #3 Slots can be edited and retargeted from the TUI
- [ ] #4 The Pins groups, placeholder rows, Pins modal, and old pin keys no longer exist
<!-- AC:END -->

## Definition of Done
<!-- DOD:BEGIN -->
- [ ] #1 `just check` passes, or `git diff --check` for docs-only changes
- [ ] #2 Docs and ADRs are updated when behavior, architecture, or workflow change
<!-- DOD:END -->
