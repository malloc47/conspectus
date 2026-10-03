---
id: CSP-172
title: Document and verify the v1 TUI workflow
status: To Do
assignee: []
created_date: '2026-05-19 03:51'
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
