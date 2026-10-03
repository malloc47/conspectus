---
id: CSP-414
title: Hybrid workspace+repo grouping in Sessions / Graph
status: Done
assignee: []
created_date: '2026-06-14 01:24'
labels:
  - h-ws
milestone: m-11
dependencies: []
ordinal: 243000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
- Follow-up (ADR 0065): `SessionsGrouping::Workspace` shipped
  and `View::Workspaces` / the `6` keybinding were removed. The
  Workspaces view's idle-workspace visibility is preserved by
  enumerating workspace nodes in the new grouping, and (B)-class
  sessions land in the Ungrouped bucket in this mode.
<!-- SECTION:DESCRIPTION:END -->

## Final Summary

<!-- SECTION:FINAL_SUMMARY:BEGIN -->
Outcome (ADR 0064): Sessions / Graph view reshaped to put
workspaces and repos at the same top level as peer parents,
each with sessions directly underneath at depth 1 (no repo
intermediate beneath workspaces, no checkout intermediate
when the repo has a single worktree). The (A)/(B)
distinction stops driving any UI marker beyond a top-level
routing decision in `resolve_group_key`: A-class sessions
route to a workspace bucket, B-class and unaffiliated route
to a repo bucket. Two long-standing operator pain points
fixed: (i) the repo level beneath workspace headers was
noise that duplicated the workspace's project context, and
(ii) agent-deck A-class sessions whose cwd is the workspace
composite directory (no checkout) dropped silently into the
"ungrouped" bucket because the legacy
`checkout_for_path(cwd)?` early return ran before the
workspace lookup. `GroupKey` now carries either a
workspace-only shape (workspace = Some, repo = None) or a
repo shape (workspace = None, repo = Some(RepoBucket));
custom `Ord` puts workspace buckets first. Workspace
headers in Graph use the shared
`format_workspace_display` helper from `rows/mod.rs` so
they read identically to the dedicated Workspaces view's
headers. `workspace_member_names` lookup uses the same
resolver-selected `WorkspaceContainsRepo` candidate links'
`logical_path` source field the Workspaces view's
`fetch_members` SQL uses, so the two surfaces stay aligned
automatically. Tests updated: `graph_grouping_uses_session_workspace_context`
expects depth 1 and 2-row tree (no repo intermediate);
renamed test `workspace_rooted_session_nests_directly_under_workspace`
checks both depth and the new workspace header format. Two
new tests cover the agent-deck workspace-root cwd case and
the hybrid peer-parents shape.
<!-- SECTION:FINAL_SUMMARY:END -->

Legacy ID: `H-WS-004`
