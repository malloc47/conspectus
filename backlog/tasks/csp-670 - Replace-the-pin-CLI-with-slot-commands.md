---
id: CSP-670
title: Replace the pin CLI with slot commands
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
priority: high
type: feature
ordinal: 728000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
The pin CLI is built around declaring and launching muxes (`create`, `launch`, `adopt`, `rebind`, `bind`). With history and rebuild, it becomes slot management. This lands with the 0.2.0 version bump, so retired verbs break cleanly instead of keeping shims.

- Scope: `pin add <ref> [--id]`, `pin edit <id> --set/--unset`, `pin rename <id> <new-id>` (lockstep tmux rename for live mux slots), `pin retarget <id> <ref>`, `pin refresh <id>`, `pin move <id> --before/--after <id>`, `pin rm`, `pin list`, `pin show`; `mux new` / `mux launch --pin`, and `--pin --no-launch` for a planned slot; retired verbs (`create`, `launch`, `attach`, `bind`, `rebind`, `adopt`) exit non-zero naming their replacement; help text updated; close CSP-617 as obsolete once `adopt` is retired.
- Tests: CLI tests for every command and each retired verb's message; snapshot tests for `pin list` and `pin show` output.
- Manual checks: run through add, edit, rename, retarget, move, and rm on a live mux slot.
- Blockers: the slot store and mux recording tickets.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 Every slot gesture from the ADR is available from the CLI, including ordering with `pin move`
- [ ] #2 Renaming a live mux slot renames its tmux session in lockstep
- [ ] #3 Each retired `pin` verb fails with a message naming its replacement
- [ ] #4 `mux new` and `mux launch` can pin the result, or create a planned slot without launching
<!-- AC:END -->

## Definition of Done
<!-- DOD:BEGIN -->
- [ ] #1 `just check` passes, or `git diff --check` for docs-only changes
- [ ] #2 Docs and ADRs are updated when behavior, architecture, or workflow change
<!-- DOD:END -->
