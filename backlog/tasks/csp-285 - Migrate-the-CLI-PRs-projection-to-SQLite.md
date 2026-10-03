---
id: CSP-285
title: Migrate the CLI PRs projection to SQLite
status: Done
assignee: []
created_date: '2026-05-26 23:06'
labels:
  - p10
milestone: m-15
dependencies:
  - CSP-282
ordinal: 478000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
- Scope: same pattern for `Projection::Pr`. Use `v_pr_by_branch`.
- Tests: parity with existing PR snapshots.
- Blockers: `CSP-282`.
<!-- SECTION:DESCRIPTION:END -->

## Final Summary

<!-- SECTION:FINAL_SUMMARY:BEGIN -->
Production renderer at `src/output/prs.rs` covers all
8 cells. The `attached` cell composes three small lookups —
preferred branch per PR (pick_strongest over
`branch_has_forge_pr` candidates from PR sources), checkout
roots per branch (active `checked_out_branch` candidates), and
agent sessions with a cwd — then matches each PR's branch's
checkout roots against agent cwd via `path_is_ancestor_of`
(lifted from `crate::model`). `strip_branch_prefix` is
promoted to `output::render`'s substrate now that two
renderers (agent + prs) need it. The in-memory `PrRowCtx`,
`pr_cell`, `pr_preferred_branch_id`, `pr_branch_label`,
`pr_attached_session_labels`, `build_pr_rows`,
`path_is_ancestor_of`, `agent_session_label`, `forge_pr_label`
are all deleted. Existing `output::table` snapshot tests are
the parity check.
<!-- SECTION:FINAL_SUMMARY:END -->

Legacy ID: `P10-007`
