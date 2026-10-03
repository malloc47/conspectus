---
id: CSP-379
title: Read-only invariant audit
status: Done
assignee: []
created_date: '2026-06-04 21:32'
labels:
  - h-pin
milestone: m-11
dependencies:
  - CSP-363
ordinal: 315000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
- Scope: explicit CLI integration tests proving `graph`,
  `node show`, `table`, `tui`, `query` never create, mtime-touch,
  or content-modify `.conspectus.toml` / user-config files
  bearing a `[pins]` section. Mirrors `CSP-063` for declared links
  and the equivalent rename audit.
- Tests: invariant tests for each command in a clean repo and a
  repo with a hand-written `[pins]` section.
- Blockers: `CSP-363`.
<!-- SECTION:DESCRIPTION:END -->

## Final Summary

<!-- SECTION:FINAL_SUMMARY:BEGIN -->
`tests/cli_pin_invariants.rs` asserts that read-only
commands do not create pin config files in a clean repo and do
not content- or mtime-touch existing project/user configs bearing
`[pins]`. Covered commands: `graph`, `table`, `query`,
`node show`, `pin list`, `pin show`, plus `tui` prelaunch
validation for the non-PTY process path; in-process TUI
navigation/read-only surfaces remain covered by reducer and UI
tests.
<!-- SECTION:FINAL_SUMMARY:END -->

Legacy ID: `H-PIN-019`
