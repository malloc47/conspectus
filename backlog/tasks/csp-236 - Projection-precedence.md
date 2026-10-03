---
id: CSP-236
title: Projection precedence
status: Done
assignee: []
created_date: '2026-05-23 02:59'
labels:
  - h-rename
milestone: m-11
dependencies:
  - CSP-235
ordinal: 285000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
- Scope: apply `alias > title > id-suffix` at the four projection sites
  — `src/output/table.rs` `title` column rendering, `src/output/node_show.rs`
  header field, `src/tui/rows/mod.rs` `AgentSessionRow` label,
  `src/tui/detail.rs` header field. Centralize the precedence helper so a
  future tweak touches one place. Snapshot fixtures updated here;
  expect non-trivial diff churn.
- Tests: projection unit tests across present-alias / present-title /
  absent-both cases at each of the four sites. Insta snapshot updates.
- Blockers: `CSP-235`.
<!-- SECTION:DESCRIPTION:END -->

Legacy ID: `H-RENAME-006`
