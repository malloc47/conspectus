---
id: CSP-259
title: '`conspectus table <ROWS>` consumes `RowFilter`'
status: Done
assignee: []
created_date: '2026-05-24 01:47'
labels:
  - f8
milestone: m-13
dependencies: []
ordinal: 455000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
- Tests: existing `filter_*_agent_table` / `filter_parity_with_tui_sessions_row_tree`
  in `src/output/table.rs` stay; added `filter_harness_narrows_mux_table_via_attached_agents`,
  `filter_mux_state_unmuxed_drops_attached_muxes`,
  `filter_parity_with_tui_mux_row_tree`, and
  `filter_union_drops_mux_rows_when_narrowing_active`. All 1684
  tests pass via `cargo nextest run --all-targets --all-features`.
- Follow-ups: config-side parity (loading `[table.<rows>].filters`
  or merging with `[tui.views.<name>]`) deferred to a separate
  story so this one stays focused on the projection layer; the
  two private `fetch_agent_mux_candidate_counts` helpers
  (`output::prs`, `output::forks`) collapse into a shared one
  when a third caller arrives.
<!-- SECTION:DESCRIPTION:END -->

## Final Summary

<!-- SECTION:FINAL_SUMMARY:BEGIN -->
Shipped. The `RowFilter` produced by `FilterArgs::to_row_filter`
in `src/cli.rs` is now applied by every `output::*` projection
builder, not just `output::agent`:
  - `output::mux` filters visible attached agents per
    `SessionMatchInputs` and drops the mux row when no
    attached agent survives, mirroring `src/tui/rows/mux.rs`'s
    `mux_matches_filter` (kept only when the filter is exactly
    `mux_state` containing `unmuxed` and the mux truly has no
    attached agents). The `agents`, `attached-count`, and
    `preview` cells now reflect the visible-only set.
  - `output::union` drops every mux row when any narrowing
    predicate is active and filters agent rows through the same
    predicate the TUI union view uses.
  - `output::prs` filters attached agents per PR and drops the
    PR row when narrowing is active and no visible attached
    agent remains.
  - `output::forks` filters resolved child agent sessions per
    fork (unresolved-target children are intentionally excluded
    once filtering is on, since the v1 dimensions need
    session-level metadata they lack), drops forks with zero
    visible children, and rewrites the `children` cell to the
    visible count. The extra `fetch_resolved_child_agents_per_fork`
    + `fetch_agent_mux_candidate_counts` lookups only run when
    the filter has narrowing predicates so the empty-filter
    path stays cheap.
Renderer wiring in `cli.rs::TableRowsArgs::run` was already in
place from CSP-258; the stale "CSP-259 will start applying"
comment on the `FilterArgs` flatten point is now corrected.
<!-- SECTION:FINAL_SUMMARY:END -->

Legacy ID: `F8-010`
