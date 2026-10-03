---
id: CSP-302
title: Add `conspectus graph --format dot`
status: Done
assignee: []
created_date: '2026-05-30 04:47'
labels:
  - gv
milestone: m-16
dependencies:
  - CSP-312
  - CSP-301
ordinal: 489000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
- Scope: add a Graphviz DOT renderer for the resolved internal graph.
  Include node kind, stable id/label, and enough styling to distinguish
  repos, checkouts, workspaces, agent sessions, mux sessions, branches,
  forks, and forge PRs. Render candidate links and resolved
  relationships distinctly so ambiguity and resolver decisions are easy
  to inspect. Once `CSP-306` exists, support rendering the named
  replay scenarios so graph visualization can be used for fixture and
  regression review without recreating local state by hand.
- Tests: deterministic DOT snapshot tests for sparse graph, mux
  candidate ambiguity, session lineage, fork ancestry, and branch→PR
  fixtures; include at least one named replay scenario when the
  scenario registry is available.
- Manual checks: run `dot -Tsvg` on at least one generated fixture and
  inspect that labels and edge kinds remain readable.
- Blockers: `CSP-312`, `CSP-301`.
<!-- SECTION:DESCRIPTION:END -->

## Final Summary

<!-- SECTION:FINAL_SUMMARY:BEGIN -->
`conspectus graph --format dot` ships alongside the
existing `--format json`, with `--candidates {include,exclude}`
and `--diagnostic-nodes {include,exclude}` flags per ADR 0050.
`conspectus dev scenario graph --format dot <name>` extends the
same surface to named replay scenarios. The renderer
(`src/output/dot.rs`) is provider-neutral: shape/fill keyed on
`NodeKind`, arrowhead on `RelationKind` category, penwidth and
color tint on `Provenance`, resolver-preferred candidates marked
`★` and solid, losing candidates dashed, ignored/overridden
candidates dashed-red with a tooltip, unresolved endpoints
rendered as dashed-circle stubs. Nodes are grouped into
per-`NodeKind` `subgraph cluster_*` blocks in fixed order;
emission is deterministic (BTreeMap node walk, sorted edges).
Snapshot coverage in `tests/dot_snapshots.rs` covers empty,
orphan-session, mux candidates (with and without
`--candidates exclude`), unresolved lineage, fork ancestry, and
branch→PR fixtures. The `process-cardinality` named scenario is
exercised structurally to assert the `--diagnostic-nodes`
filter. Manually verified with `dot -Tsvg` on the `exact-match`,
`ambiguous-mux`, and `fork-lineage` scenarios.
<!-- SECTION:FINAL_SUMMARY:END -->

Legacy ID: `GV-002`
