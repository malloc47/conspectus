---
id: CSP-019
title: Add representative graph JSON snapshots
status: Done
assignee: []
created_date: '2026-05-15 02:36'
labels:
  - p1
milestone: m-2
dependencies:
  - CSP-014
  - CSP-015
  - CSP-017
  - CSP-018
ordinal: 19000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
- Scope: snapshot empty graph JSON and sparse fixtures covering orphan
  session, mux-only, repo-only, unresolved lineage, conflicts, and mux
  candidates.
- Tests: `cargo test --all-targets --all-features`; `cargo nextest run
  --all-targets --all-features`.
- Manual checks: review snapshots for stable ordering and public shape.
- Blockers: `CSP-014`, `CSP-015`, `CSP-017`, `CSP-018`.
<!-- SECTION:DESCRIPTION:END -->

Legacy ID: `P1-009`
