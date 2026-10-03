---
id: CSP-099
title: Implement the bootstrap-roots flow described in the design
status: To Do
assignee: []
created_date: '2026-05-17 20:45'
labels:
  - h-prod
milestone: m-11
dependencies:
  - CSP-107
ordinal: 132000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
- Scope: `docs/design.md` "Discovery Strategy" mentions a future
  bootstrap mode that prints suggested roots and links by default and
  requires an explicit write flag to persist. Add `conspectus bootstrap`
  (or `conspectus graph --suggest-roots`) that scans selected
  directories and prints a TOML stanza, with `--write` controlling
  persistence.
- Tests: CLI integration tests for suggested output, `--write` behavior,
  and read-only defaults.
- Blockers: `CSP-107` for the persistence target.
<!-- SECTION:DESCRIPTION:END -->

Legacy ID: `H-PROD-001`
