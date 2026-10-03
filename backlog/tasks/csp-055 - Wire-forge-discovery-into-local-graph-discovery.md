---
id: CSP-055
title: Wire forge discovery into local graph discovery
status: Done
assignee: []
created_date: '2026-05-16 03:21'
labels:
  - p4
milestone: m-5
dependencies:
  - CSP-047
  - CSP-048
ordinal: 53000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
- Scope: register the forge provider in `discover_local_with` behind
  `LocalDiscoveryConfig::forge_runner` (mirroring the tmux pattern).
  `from_env()` builds a real `SystemGh` runner unless
  `CONSPECTUS_DISABLE_FORGE` is set. Discovery remains best-effort:
  missing / unauthenticated `gh` degrades to no PR data instead of
  failing the run. Cross-link inference passes PR evidence through
  `cross_link::infer` so ambiguous branch ↔ PR matches remain visible.
- Tests: library tests for the wired path with a `FakeGh` runner;
  CLI integration tests with the forge provider disabled and with
  a fake `gh` output.
- Manual checks: run `cargo run -- graph --format json` from a repo
  with an open PR and confirm the `ForgePr` node and link appear.
- Blockers: `CSP-047`, `CSP-048`.
<!-- SECTION:DESCRIPTION:END -->

## Final Summary

<!-- SECTION:FINAL_SUMMARY:BEGIN -->
Added a `GitHubForgeProvider` that probes each scan
root with `GitProbe`, extracts host/owner/repo from a
GitHub-shaped git remote, runs `gh pr list --json` via the
injected `GhRunner`, parses the rows, and emits a
`fragment_for_repo`. A `parse_github_remote` helper covers
`https://`, `git@`, `ssh://`, and GitHub-Enterprise hosts and
rejects non-GitHub URLs. `LocalDiscoveryConfig` gained a
`forge_runner` slot mirroring `tmux_runner`; `from_env()`
builds a real `SystemGh` runner unless
`CONSPECTUS_DISABLE_FORGE` is set. Unavailable / failed `gh`
outcomes degrade silently. CLI smoke tests set
`CONSPECTUS_DISABLE_FORGE=1` to keep tests offline; new library
tests cover the wired path with `FakeGh` and the "no runner"
case.
<!-- SECTION:FINAL_SUMMARY:END -->

Legacy ID: `P4-011`
