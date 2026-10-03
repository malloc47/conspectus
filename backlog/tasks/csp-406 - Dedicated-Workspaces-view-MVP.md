---
id: CSP-406
title: Dedicated Workspaces view (MVP)
status: Done
assignee: []
created_date: '2026-06-09 14:15'
labels:
  - h-ws
milestone: m-11
dependencies: []
ordinal: 240000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
- Polish (ADR 0062): operator feedback after running the MVP
  flagged the `members` subgroup as left-tree noise (the detail
  pane already exposes members as navigable HeaderFields) and
  the `in workspace` / `related` labels as unintuitive. The
  polish pass moved the member list inline on the workspace
  top-level row as a `+`-joined span (matching the agent-table
  workspace column from ADR 0060) and dropped the `related`
  (B)-class subgroup entirely. The Workspaces view now surfaces
  only (A)-class sessions, sitting at depth 1 directly under the
  workspace row with no labeled wrapper. The (B) cross-reference
  signal continues to live as the `[ws-name]` chip in Sessions /
  Graph from `CSP-405`. `fetch_b_class_sessions` and the
  multi-hop join it powered are removed; `MemberSqlRow` slims
  to a single `display_name` field. Six unit tests cover empty
  snapshot, inline member-list rendering with and without
  provider, (A)-class direct nesting at depth 1, (B)-class
  suppression, and logical-path-basename naming.
- ADR: 0062 records the polish decision and answers
  open question 2 from
  `docs/plans/workspace-view-redesign.md` (the (B)-in-view
  question). The four-grouping menu decision is still
  `CSP-406.01`'s.
<!-- SECTION:DESCRIPTION:END -->

## Final Summary

<!-- SECTION:FINAL_SUMMARY:BEGIN -->
New `View::Workspaces` with `WorkspacesGrouping::Flat`
as the only grouping shipped in v1. Initial row tree
(`src/tui/rows/workspaces.rs`) listed each workspace with up to
three labeled subgroups (`members` / `in workspace` /
`related`). Keybinding `6` switches to the view; `[`/`]` cycle
includes Workspaces; `--view workspaces` works from the CLI.
Default-collapse for the `related` subgroup and the Provider /
Activity / Repo groupings were deferred to `CSP-406.01`.
<!-- SECTION:FINAL_SUMMARY:END -->

Legacy ID: `H-WS-002`
