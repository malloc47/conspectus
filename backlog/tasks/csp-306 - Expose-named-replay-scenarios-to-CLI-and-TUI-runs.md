---
id: CSP-306
title: Expose named replay scenarios to CLI and TUI runs
status: Done
assignee: []
created_date: '2026-05-31 03:39'
labels:
  - test
milestone: m-11
dependencies:
  - CSP-262
ordinal: 278000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
- Scope: promote the replay harness's useful worlds into a small
  named scenario registry shared by tests and developer commands.
  Each scenario should materialize an isolated temp world and return
  enough launch context for `graph`, `table`, `node show`, and
  `tui` to run against exactly the same generated state. Cover at
  least: empty world, orphan session, session+tmux exact match,
  ambiguous mux candidates, hook supersession, Codex fd-beats-stale
  argv, workspace with PR, and fork lineage. Keep the scenario
  materializer test-only or explicitly gated so production discovery
  does not grow fixture behavior.
- Tests: scenario-registry tests proving every named scenario can
  materialize, run discovery, resolve, render graph JSON, render the
  relevant table row-type, and build the TUI row tree without reading
  the user's home directory, real tmux, real `/proc`, or network.
- Manual checks: add a documented launch path such as
  `conspectus dev scenario tui ambiguous-mux` (exact surface to be
  decided during implementation) and verify it opens the real TUI on
  the generated scenario. Also verify graph/table output from the
  same scenario matches the automated snapshots.
- Blockers: `CSP-262`; useful before `CSP-266` and `CSP-302`/`CSP-303`
  so interaction tests and visualization exports share scenario
  names instead of rebuilding fixtures independently.
<!-- SECTION:DESCRIPTION:END -->

## Final Summary

<!-- SECTION:FINAL_SUMMARY:BEGIN -->
Added a debug/test-only `dev_scenarios` module with
named worlds for empty, orphan-session, exact-match,
ambiguous-mux, hook-supersession, Codex fd-current,
workspace-pr, and fork-lineage cases. Added hidden debug CLI
commands under `conspectus dev scenario` for list, graph,
table, node, and static TUI launch. Scenario tests prove every
world materializes, resolves, renders graph JSON and table output,
and builds a TUI sessions row tree without reading real home,
tmux, `/proc`, or network state.
<!-- SECTION:FINAL_SUMMARY:END -->

Legacy ID: `TEST-006`
