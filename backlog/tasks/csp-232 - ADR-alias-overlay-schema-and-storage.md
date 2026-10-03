---
id: CSP-232
title: 'ADR: alias overlay schema and storage'
status: Done
assignee: []
created_date: '2026-05-23 02:59'
labels:
  - h-rename
milestone: m-11
dependencies: []
ordinal: 281000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
- Scope: settle the storage schema (`[[aliases]]` table sibling to
  `[declared]`, not nested inside it), store-selection rules, conflict
  resolution between local and global, render precedence
  (`alias > title > id-suffix`), mux node-id stability rule (no mux
  aliases stored — lockstep renames mutate the native tmux name
  directly), lockstep contract, hook-sidecar drift caveat, and the
  alias-equals-title round-trip rule. Record as ADR 0029.
- Tests: docs-only; `git diff --check`.
- Blockers: none.
<!-- SECTION:DESCRIPTION:END -->

Legacy ID: `H-RENAME-001`
