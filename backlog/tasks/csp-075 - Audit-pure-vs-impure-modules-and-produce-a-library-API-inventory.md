---
id: CSP-075
title: Audit pure vs impure modules and produce a library API inventory
status: Done
assignee: []
created_date: '2026-05-16 16:30'
labels:
  - p6
milestone: m-7
dependencies:
  - CSP-073
ordinal: 70000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
- Scope: walk every module under `src/` and tag it as either
  pure (no `std::env`, `std::process`, `current_dir`, no global
  state) or impure boundary code, and write the result up as
  `docs/library-api.md`. Identify entry points consumers should
  call (e.g. `discover_local_with`, `resolve_snapshot`,
  `render_graph_json`, `output::table::render`, the declared-link
  read/write helpers) and call out the impure seams
  (`*::from_env`, the runners) so consumers know what they have to
  inject to keep things testable.
- Tests: docs-only; `git diff --check`.
- Manual checks: re-grep for `std::env`, `std::process`, and
  `current_dir` after the audit and confirm the inventory matches.
- Blockers: `CSP-073`.
<!-- SECTION:DESCRIPTION:END -->

## Final Summary

<!-- SECTION:FINAL_SUMMARY:BEGIN -->
Added `docs/library-api.md` with the stable consumer
workflow, pure-module inventory, impure boundary inventory,
injection guidance, stable entry points, and environment toggles.
The source audit found production process boundaries in git, tmux,
and gh runners, and current-directory/environment boundaries in CLI
helpers, `DiscoveryContext::from_current_dir`,
`LocalDiscoveryConfig::from_env`, and `ConfigLoader::from_env`.
<!-- SECTION:FINAL_SUMMARY:END -->

Legacy ID: `P6-003`
