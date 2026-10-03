---
id: CSP-276
title: 'Saved views: a library of common queries'
status: Done
assignee: []
created_date: '2026-05-25 21:41'
labels:
  - p9
milestone: m-14
dependencies:
  - CSP-272
ordinal: 468000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
- Scope: ship a small set of named views (CREATE VIEW under the
  DDL) that name the joins users would write by hand:
  `v_sessions_with_repo`, `v_mux_attachments`, `v_pr_by_branch`,
  `v_fork_ancestry` (recursive), `v_workspace_member_repos`. The
  view set is small and curated — not a contract. Document each in
  `docs/query-guide.md`. `conspectus query --list-views` enumerates
  them.
- Tests: each named view selectable; query against each returns
  expected fixture rows; `--list-views` snapshot test.
- Manual checks: `conspectus query 'SELECT * FROM v_fork_ancestry'`
  on a populated graph.
- Blockers: `CSP-272`.
<!-- SECTION:DESCRIPTION:END -->

## Final Summary

<!-- SECTION:FINAL_SUMMARY:BEGIN -->
Schema and registry now ship `v_sessions_with_repo`,
`v_mux_attachments`, `v_pr_by_branch`, `v_fork_ancestry`, and
`v_workspace_member_repos`; `conspectus query --list-views`
renders the curated registry. `docs/query-guide.md` documents the
views, and query regression snapshots exercise each saved view.
<!-- SECTION:FINAL_SUMMARY:END -->

Legacy ID: `P9-006`
