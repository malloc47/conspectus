---
id: CSP-467
title: Introduce a `SnapshotIndex` for graph lookups
status: Done
assignee: []
created_date: '2026-07-01 22:56'
labels:
  - h-hyg
milestone: m-11
dependencies:
  - CSP-463
  - CSP-495
ordinal: 98000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
- **Waves 1–3, 5, 6, 7 landed 2026-07-04..05**:
  * Wave 1 (`e952bc4`): `SnapshotIndex<'a>` struct +
    `new(&snapshot)` builder + `id_to_node` map + consistency
    unit test (`snapshot_index_agrees_with_linear_scan_on_dense_fixture`).
  * Wave 2 (`29c781a`): `agent_mux_candidate_counts()` map
    + migrated 5 consumers (`tui/rows/{union,prs,forks}.rs`
    + `output/{prs,forks}.rs`). Retired
    `tui::rows::collect_agent_mux_candidate_counts` — the
    CSP-463 interim home is gone. `rows/mux.rs` retains a
    local copy per CSP-463's "near-twin stays put" scope
    (wave 5 retires it too).
  * Wave 3 (`a840fc1`): `links_for(source, relation) →
    &[&GraphLink]` (source_node → links-by-relation index).
    Substrate only — no callers migrated yet.
  * Wave 5 (`7817c41`): retired the last local copy of
    `collect_agent_mux_candidate_counts` — the near-twin in
    `tui/rows/mux.rs` that CSP-463 explicitly left in place.
    The `CSP-467` interim `collect_agent_mux_candidate_counts`
    helper family is fully gone.
  * Wave 6 (`267a1b1`): added `links_with_relation(relation)`
    + `link(link_id)` maps to `SnapshotIndex`. Every hot
    linear-scan shape the audit called out is now covered
    by the index.
  * Wave 7 (`eb0d6ae`): migrated 4 clean by-source-relation
    / by-link-id scan sites (`declared::fork_root`,
    `declared::branch_for_pr`, `tui/detail::workspace_member_fields`,
    `tui/detail::diagnostic_summaries`) to consult
    `SnapshotIndex` instead of raw `candidate_links.iter()`.
- **Wave 4 (preferred-mux per session lookup)** stays deferred
  to `CSP-495`: depends on the resolver's per-session mux
  picker which is currently scoped as "resolver semantics —
  out of scope for H-HYG" per the audit. Lands alongside
  `CSP-495` when that story lifts the row trees off
  `RunConfig` and into derived view-models.
  * Wave 6 (SnapshotIndex expansion): the 25 remaining
    `snapshot.candidate_links.iter()` scans use shapes the
    wave-3 `(source, relation)` index doesn't cover
    (by-link-id, by-target, resolver-selected). Each needs a
    focused per-shape index expansion; land as demand-driven
    commits when a hot consumer needs them.
- Blockers: `CSP-463` landed. `CSP-495` builds on this
  substrate to make row trees fully derived view-models.
<!-- SECTION:DESCRIPTION:END -->

Legacy ID: `H-HYG-006`
