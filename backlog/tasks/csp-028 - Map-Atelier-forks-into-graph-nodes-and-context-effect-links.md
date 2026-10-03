---
id: CSP-028
title: Map Atelier forks into graph nodes and context-effect links
status: Done
assignee: []
created_date: '2026-05-15 03:58'
labels:
  - p2
milestone: m-3
dependencies:
  - CSP-027
ordinal: 28000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
- Scope: emit one polymorphic `Fork` node per provider fork and candidate
  links for `forks_workspace`, `forks_repo`, `created_checkout`,
  `referenced_checkout`, `created_branch`, `associated_branch`,
  `rooted_at_path`, and `parent_fork` where evidence exists.
- Tests: resolver and snapshot tests for created vs referenced checkouts,
  research forks, selected forks, standalone repo forks, parent forks, and
  associated branch links.
- Manual checks: inspect graph JSON from Atelier fork fixtures and confirm no
  fake workspace nodes are fabricated for standalone repo contexts.
- Blockers: `CSP-027`.
<!-- SECTION:DESCRIPTION:END -->

## Final Summary

<!-- SECTION:FINAL_SUMMARY:BEGIN -->
Emitted one `Fork` node per Atelier fork plus candidate links for
workspace scope, repo scope, created checkouts, referenced checkouts,
created or associated branches, fork roots as unresolved path evidence, and
parent forks, with snapshot coverage for worktree, selected, research, and
standalone contexts.
<!-- SECTION:FINAL_SUMMARY:END -->

Legacy ID: `P2-008`
