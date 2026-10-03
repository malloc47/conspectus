---
id: CSP-283
title: Migrate the CLI mux projection to SQLite
status: Done
assignee: []
created_date: '2026-05-26 23:06'
labels:
  - p10
milestone: m-15
dependencies:
  - CSP-282
ordinal: 476000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
- Scope: same pattern as `CSP-282` for `Projection::Mux`. Use
  `v_mux_attachments` (extended if needed) for the agents-attached-
  to-this-mux cell.
- Tests: parity with the existing mux-projection snapshots.
- Blockers: `CSP-282` (substrate validated by the agent migration).
<!-- SECTION:DESCRIPTION:END -->

## Final Summary

<!-- SECTION:FINAL_SUMMARY:BEGIN -->
Production renderer at `src/output/mux.rs` covers all
8 cells (`id`, `mux`, `cwd`, `agents`, `preview`,
`attached-count`, `activity`, `created`). One primary query
(`SELECT … FROM node_mux_sessions`) plus a per-mux attachment
lookup that mirrors `SnapshotView::attached_to_mux` (pick the
preferred `linked_to_mux` candidate per source agent, group by
mux `node_id`, preserve BTreeMap-by-source ordering) and a
per-agent ambiguity count for the per-attachment indicator's
`*` marker. `attached_to_mux` is removed from `SnapshotView`;
the in-memory `mux_cell` / `build_mux_rows` /
`first_attached_agent_preview` / `MuxRowCtx` are deleted. The
`v_mux_attachments` saved view didn't need extension — the
renderer's queries are inlined since the view's columns don't
include `preview` and adding it would broaden the view's
contract beyond what callers asked for. Existing
`output::table` snapshot tests are the parity check; all 731
lib tests pass byte-for-byte, full integration suite green.
<!-- SECTION:FINAL_SUMMARY:END -->

Legacy ID: `P10-005`
