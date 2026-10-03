---
id: CSP-381
title: Docs and operations guide
status: Done
assignee: []
created_date: '2026-06-04 21:32'
labels:
  - h-pin
milestone: m-11
dependencies:
  - CSP-372
  - CSP-377
  - CSP-378
  - CSP-387
  - CSP-388
  - CSP-389
ordinal: 317000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
- Scope: update `docs/operations.md` and `README.md` with the
  `conspectus pin` command surface, the agent-deck migration path
  via `pin adopt`, and the read-only invariant. Update the Phase 8
  TUI doc with pin keybindings. Cross-link from `docs/design.md`
  Session Pins section to operations doc once it exists. This can
  run in parallel with implementation; final closeout should add
  the Controls overlay CRUD details from `CSP-387..389` before
  promoting ADR 0057 from Proposed to Accepted.
- Tests: doctest where applicable; `git diff --check`; insta
  review.
- Blockers: none for the initial docs slice. Final closeout waits
  on `CSP-372`, `CSP-377`, `CSP-378`, `CSP-387`,
  `CSP-388`, `CSP-389`.
<!-- SECTION:DESCRIPTION:END -->

## Final Summary

<!-- SECTION:FINAL_SUMMARY:BEGIN -->
README and operations docs describe the `conspectus pin`
command surface, agent-deck migration via `pin adopt`,
read-only invariants, row-level TUI actions, and Controls overlay
create/edit/remove/bind/rebind/adopt flows. `docs/design.md`
links the Session Pins section to the operations guide, and ADR
0057 is promoted to Accepted.
<!-- SECTION:FINAL_SUMMARY:END -->

Legacy ID: `H-PIN-021`
