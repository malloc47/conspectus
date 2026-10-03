---
id: CSP-286
title: Migrate the CLI forks projection to SQLite
status: Done
assignee: []
created_date: '2026-05-26 23:06'
labels:
  - p10
milestone: m-15
dependencies:
  - CSP-282
ordinal: 479000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
- Scope: same pattern for `Projection::Fork`. Use
  `v_fork_ancestry`.
- Tests: parity with existing fork snapshots.
- Blockers: `CSP-282`.
<!-- SECTION:DESCRIPTION:END -->

## Final Summary

<!-- SECTION:FINAL_SUMMARY:BEGIN -->
Production renderer at `src/output/forks.rs` covers
all 7 cells. Primary query against `node_forks`, plus
side-lookups for `parent` (pick_strongest over
`parent_session` candidates with the same resolved-vs-unresolved
label branching the in-memory `fork_parent_session_label`
used) and `children` (a GROUP BY count of active
`child_session` candidates targeting agent_session nodes or
unresolved endpoints). `v_fork_ancestry` was not needed for
this projection — that view supports recursive parent walks,
not per-fork rendering. The in-memory `ForkRowCtx`,
`fork_cell`, `fork_label`, `fork_parent_session_label`,
`fork_child_session_count`, `build_fork_rows` are deleted.
With this story, `render_with` no longer constructs a
`SnapshotView` for any projection — the struct, its `Deref`
impl, and the `SnapshotView::new` helper are all removed from
`output::table`. Existing `output::table` snapshot tests are
the parity check; all 731 lib tests pass byte-for-byte, full
integration suite green.
<!-- SECTION:FINAL_SUMMARY:END -->

Legacy ID: `P10-008`
