---
id: CSP-005
title: Add the thin CLI surface
status: Done
assignee: []
created_date: '2026-05-15 02:31'
labels:
  - p0
milestone: m-1
dependencies:
  - CSP-004
ordinal: 5000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
- Scope: wire `clap` so `conspectus --help` and `conspectus --version`
  work without implementing graph discovery.
- Tests: CLI smoke tests for `--help` and `--version`.
- Manual checks: `cargo run -- --help`; `cargo run -- --version`.
- Blockers: `CSP-004`.
<!-- SECTION:DESCRIPTION:END -->

Legacy ID: `P0-002`
