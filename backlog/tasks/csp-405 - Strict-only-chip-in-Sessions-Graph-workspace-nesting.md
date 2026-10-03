---
id: CSP-405
title: Strict-only + chip in Sessions/Graph workspace nesting
status: Done
assignee: []
created_date: '2026-06-09 14:15'
labels:
  - h-ws
milestone: m-11
dependencies: []
ordinal: 239000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
- Chip half — reverted (ADR 0063): the original ship added a
  `[ws-name]` / `[N ws]` cross-reference chip on (B)-class rows
  via `workspaces_for_repo`, `active_workspaces`, and
  `weak_workspace_chip` helpers, with an activeness gate and
  `WEAK_WORKSPACE_CHIP_MAX = 3` cardinality threshold. After
  running the chip the operator reported it was not surfacing
  actionable information — knowing that a session's repo
  *happens* to be claimed by a workspace, without the session
  being workspace-rooted, did not change any decision the
  operator made. The chip, its helpers, the
  `workspace_chip: Option<String>` field on `AgentSessionRow`,
  the rendering block in `render_session_spans`, and the four
  chip-specific tests are removed. The (B)-rendering test
  keeps only the strict-nesting depth assertions.
- Net result: the CSP-405 contribution is the strict-nesting
  + cross-link inference fix; the cross-reference chip is gone
  from the UI in all five views (Mux/Prs/Forks/Union were
  already chipless per ADR 0061; Workspaces dropped the
  equivalent `related` subgroup per ADR 0062). The (A)/(B)
  distinction remains load-bearing at the data-model layer
  (the AssociatedWith inference still emits it) but is no
  longer surfaced anywhere in the TUI.
<!-- SECTION:DESCRIPTION:END -->

## Final Summary

<!-- SECTION:FINAL_SUMMARY:BEGIN -->
Outcome (strict-nesting half — retained): `resolve_group_key`
sets the workspace level only when the session carries a
direct `AssociatedWith Workspace` edge; the previous
`workspace_for_repo` fallback is removed. Repo-shared
(B-class) sessions fall through to repo-level grouping.
`cross_link::workspace_member_roots` indexes both the
workspace's own `root` and every member's `logical_path`
(dropping `canonical_checkout_root` to fix the symlinked-member
leak), with deepest-match-wins keeping member-subdir
attribution preferred. Regression test
`symlinked_workspace_member_does_not_associate_session_at_canonical_path`
in `discovery::cross_link::tests` pins the corrected
semantics. Strict-nesting tests
(`workspace_rooted_session_nests_under_workspace_at_depth_2`,
`repo_shared_session_stays_at_repo_level`) cover the bug-fix
outcome.
<!-- SECTION:FINAL_SUMMARY:END -->

Legacy ID: `H-WS-001`
