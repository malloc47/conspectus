---
id: CSP-377
title: TUI keybindings for pin actions
status: Done
assignee: []
created_date: '2026-06-04 21:32'
labels:
  - h-pin
milestone: m-11
dependencies:
  - CSP-376
ordinal: 310000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
- Scope: bind `Enter` on a pin row to launch (unbound) or attach
  (bound) via CSP-372; `R` to rename (lockstep via ADR 0029);
  `Delete` to remove with confirmation. Add a Pins action group
  placeholder to the ADR 0031 Controls overlay that opens the
  richer CRUD flows tracked in `CSP-387..389`. Decide the
  bound-pin glyph in coordination with ADR 0032's theme
  vocabulary.
- Tests: reducer tests for the new keys; snapshot tests for the
  Controls overlay open state with the Pins group.
- Blockers: `CSP-376`.
<!-- SECTION:DESCRIPTION:END -->

## Final Summary

<!-- SECTION:FINAL_SUMMARY:BEGIN -->
`Enter` on a pin row shells out to `conspectus pin
launch <id>` and refreshes on return; `R` opens the existing
text-input overlay for pin display-name edits and commits via
`conspectus pin rename --display`; `Delete` uses a second-press
confirmation before `conspectus pin rm`; static scenario TUIs
keep these mutating actions disabled. Controls overlay includes
a discoverable Pins action group whose structured CRUD editors
remain in `CSP-387..389`.
<!-- SECTION:FINAL_SUMMARY:END -->

Legacy ID: `H-PIN-017`
