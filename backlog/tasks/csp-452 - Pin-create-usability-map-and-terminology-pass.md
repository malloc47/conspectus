---
id: CSP-452
title: Pin create usability map and terminology pass
status: Done
assignee: []
created_date: '2026-06-24 20:11'
labels:
  - h-pin-tui
milestone: m-11
dependencies: []
ordinal: 318000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
- Scope: review the current TUI pin create form as an operator
  workflow, not a schema editor. Decide the user-facing labels and
  field order for the create modal: replace or visually subordinate
  implementation-facing `id` with a primary `name` field; group
  derived identity fields (`display_name`, `mux.name`, persisted
  id) behind the name-sync behavior in `CSP-453`; make the
  launch command and target store visible before confirmation.
  Record any copy/keybinding changes in `docs/operations.md` and
  the TUI help text.
- Tests: docs-only for the first slice; `git diff --check`.
- Blockers: none.
<!-- SECTION:DESCRIPTION:END -->

## Final Summary

<!-- SECTION:FINAL_SUMMARY:BEGIN -->
Product target recorded for the remaining
`H-PIN-TUI-*` stories. The create flow should be framed around a
primary `name`, selected-row context, and an explicit mode toggle
inside one create flow: `adopt selected` for pinning a running
mux/session exactly, or `new` for creating a fresh session/mux
from the same cwd/workspace. Target field order:
`name`, mode/context summary, cwd, harness, launch command
preview, advanced identity fields (persisted id, display name,
mux name/socket), store, then confirm. `id` remains a persisted
schema field but should no
longer be the first user-facing concept. Runtime copy/help changes
are intentionally left to `CSP-453` and
`CSP-454`, where the form state and key behavior actually
change.
<!-- SECTION:FINAL_SUMMARY:END -->

Legacy ID: `H-PIN-TUI-001`
