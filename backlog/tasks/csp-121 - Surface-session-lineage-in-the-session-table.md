---
id: CSP-121
title: Surface session lineage in the session table
status: Done
assignee: []
created_date: '2026-05-17 20:45'
labels:
  - h-lineage
milestone: m-11
dependencies: []
ordinal: 184000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
- Resolution: `src/output/table.rs` now adds a `LINEAGE` column to
  the agent projection. The cell shows the preferred
  `parent_session`'s short id (full when ≤12 chars, else `…<last-8>`
  for UUIDs), prefixes unresolved parents with `?`, and appends `←`
  when the parent itself has a parent (chain ≥ 2). Unit tests cover
  resolved-parent, multi-level chain, and unresolved-parent cases.
  The `--include-superseded` flag was deferred: with fork-shaped
  lineage (codex `forked_from_id`, atelier-style native forks) a
  single parent can have multiple children and would silently
  disappear from every default render, which is more surprising than
  showing all rows. JSON output is already exhaustive. If a future
  consumer needs a compressed view, add `--hide-superseded` then.
<!-- SECTION:DESCRIPTION:END -->

Legacy ID: `H-LINEAGE-005`
