---
id: CSP-009
title: Add project-foundation smoke tests
status: Done
assignee: []
created_date: '2026-05-15 02:31'
labels:
  - p0
milestone: m-1
dependencies:
  - CSP-005
  - CSP-007
  - CSP-008
ordinal: 9000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
- Scope: add CLI integration tests covering `--help` and `--version`, and
  make them part of the baseline check flow.
- Tests: `cargo test --all-targets --all-features`; `cargo nextest run
  --all-targets --all-features`.
- Manual checks: run both CLI commands directly.
- Blockers: `CSP-005`, `CSP-007`, `CSP-008`.
<!-- SECTION:DESCRIPTION:END -->

Legacy ID: `P0-006`
