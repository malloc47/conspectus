---
id: CSP-242
title: Read-only invariant audit
status: Done
assignee: []
created_date: '2026-05-23 02:59'
labels:
  - h-rename
milestone: m-11
dependencies:
  - CSP-241
ordinal: 291000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
- Scope: mirror of `CSP-063`. Smoke tests verifying that `conspectus
  graph`, `conspectus node show`, `conspectus table`, and TUI
  navigation (no rename action) do not mtime-touch or content-modify
  alias-bearing config files. Add to the existing read-only test
  harness used by Phase 5.
- Tests: as scoped above.
- Blockers: `CSP-241`.
<!-- SECTION:DESCRIPTION:END -->

Legacy ID: `H-RENAME-012`
