---
id: CSP-313
title: Model detail-pane relationship groups and previews
status: Done
assignee: []
created_date: '2026-06-01 00:28'
labels:
  - t8
milestone: m-13
dependencies:
  - CSP-312
  - CSP-288
ordinal: 417000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
- Scope: replace the recursive `HeaderField.expanded_fields` detail
  payload with a view model that separates core node facts,
  relationship groups, selected relationship row, neighbor preview,
  and breadcrumbs. Relationship rows should carry direction,
  relation kind, neighbor node id/kind/label, evidence, confidence,
  state, selected-link id, and resolved-vs-candidate context. Keep
  the model SQLite-backed through `build_node_detail_from_conn`.
- Tests: pure view-model tests for mux, agent session, runtime
  process, repo/checkout, fork, and PR nodes; coverage for empty
  groups, unresolved endpoints, conflicts, and long labels.
- Blockers: `CSP-312`, `CSP-288`.
<!-- SECTION:DESCRIPTION:END -->

Legacy ID: `T8-027`
