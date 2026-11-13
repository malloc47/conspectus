---
id: CSP-172
title: Document and verify the v1 TUI workflow
status: To Do
assignee: []
created_date: '2026-05-19 03:51'
updated_date: '2026-10-31 08:15'
labels:
  - p8
milestone: m-13
dependencies:
  - CSP-167
  - CSP-169
  - CSP-170
  - CSP-171.01
  - CSP-171.02
ordinal: 402000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
- Scope: update `docs/operations.md` and README-level command listings
  with `conspectus tui`, keybindings, privacy/performance notes for live
  preview, supported actions, unsupported actions, and troubleshooting
  for terminal cleanup. Run the full automated suite and the manual
  checks from the Phase 8 implementation document.
- Tests: `just check`; targeted TUI snapshot tests; CLI smoke tests.
- Manual checks: all commands listed in
  `docs/implementation/phase-08-interactive-tui.md`.
- Blockers: `CSP-167`, `CSP-169`; `CSP-170`, `CSP-171.01`, `CSP-171.02`, and
  `CSP-171.03` if included in the v1 release boundary.
<!-- SECTION:DESCRIPTION:END -->

Legacy ID: `P8-013`

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
TUI framework update (2026-10-31, CSP-766): the main screen's keys are listed by the `?` page, generated from the scoped keymap (`src/tui/keybindings.rs`, ADR 0192), and each overlay's footer and help come from its key table (ADR 0162, ADR 0193); `docs/operations.md` gained "What changed in the TUI" (CSP-764). Point operators at `?` rather than copying key tables into the docs.
<!-- SECTION:NOTES:END -->
