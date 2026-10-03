---
id: CSP-329
title: Emit explicit resolver decision rationale
status: To Do
assignee: []
created_date: '2026-06-02 03:59'
labels:
  - gv
milestone: m-16
dependencies: []
ordinal: 496000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
- Scope: today the resolver picks a winner per candidate group by
  running `compare_session_mux` / `compare_branch_pr` /
  `compare_generic` and dropping the result into
  `ResolvedRelationship.selected_link_id`. The HTML inspector
  shows the winner side-by-side with `competing_link_ids` so an
  operator can deduce the reason from the provenance / fields
  diff. Make the resolver emit the reason directly: a short
  string per resolution such as `"local_declared beat
  strong_discovered by provenance precedence"`, `"strong_discovered
  beat convention by mux tier"`, `"tied — broken by link id sort"`.
  Add `selected_reason: Option<String>` to `ResolvedRelationship`
  (and the SQLite materialization), thread through the HTML
  payload, and render in the inspector above the lost-candidates
  list.
- Tests: extend the resolver unit tests with explicit-reason
  assertions for each comparator branch; refresh fixture
  snapshots and HTML payload snapshots that lock in the field.
- Manual checks: render the named scenarios that exercise each
  comparator (ambiguous-mux for mux tier, workspace-pr for PR
  selection, etc.) and confirm the inspector text reads
  correctly.
- Blockers: none. Touches `src/resolve/`, `src/output/html/`,
  `src/output/dot.rs` (optional: render reason in the DOT edge
  tooltip), and `src/query/` (selected_reason column).
<!-- SECTION:DESCRIPTION:END -->

Legacy ID: `GV-EDGEREASON`
