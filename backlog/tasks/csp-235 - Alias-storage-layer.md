---
id: CSP-235
title: Alias storage layer
status: Done
assignee: []
created_date: '2026-05-23 02:59'
labels:
  - h-rename
milestone: m-11
dependencies:
  - CSP-232
ordinal: 284000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
- Scope: per ADR 0029. Define the TOML model (round-trip), load aliases
  from local + global stores at discovery time into a sidecar
  `HashMap<NodeId, String>` carried alongside the graph snapshot. Add
  atomic write helpers analogous to `upsert_declared_link` and
  `remove_declared_link` (`src/declared.rs:233`, `:252`); reuse
  `write_atomic` (`src/declared.rs:400-430`) directly. Reuse
  `select_store_for_declaration` (`src/declared.rs:143-167`) for nearest-
  store write selection. Reuse `DeclaredEndpoint` (`src/declared.rs:77`)
  as the node-key encoding.
- Tests: round-trip TOML tests for the new schema, store-selection
  tests across project-rooted vs orphan agent sessions, malformed-entry
  diagnostics, schema-version skip behavior, atomic-write retry path.
- Blockers: `CSP-232`.
<!-- SECTION:DESCRIPTION:END -->

Legacy ID: `H-RENAME-004`
