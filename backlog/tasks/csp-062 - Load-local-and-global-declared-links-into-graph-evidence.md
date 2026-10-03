---
id: CSP-062
title: Load local and global declared links into graph evidence
status: Done
assignee: []
created_date: '2026-05-16 04:29'
labels:
  - p5
milestone: m-6
dependencies:
  - CSP-061
ordinal: 58000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
- Scope: teach local discovery to read project `.conspectus.toml`
  and user config declared-link sections without writing either file,
  convert entries into `GraphLink` candidates with
  `LocalDeclared` / `GlobalDeclared` provenance, and preserve
  unresolved endpoint evidence when a declared endpoint is not present
  in the current graph.
- Tests: unit/library tests for local-only, global-only,
  local-over-global, unresolved endpoints, ignored entries,
  overridden entries, malformed files producing diagnostics, and
  discovery over roots with no config files.
- Manual checks: run `cargo run -- graph --format json` in a repo
  with hand-written `.conspectus.toml` declarations and inspect
  provenance, link state, diagnostics, and resolved relationships.
- Blockers: `CSP-061`.
<!-- SECTION:DESCRIPTION:END -->

## Final Summary

<!-- SECTION:FINAL_SUMMARY:BEGIN -->
Added a read-only `discovery::declared` pass that loads
user config and per-root project config, maps entries into
`GraphLink` candidates with `LocalDeclared` / `GlobalDeclared`
provenance, resolves declared targets against the discovered node
set when present, preserves missing targets as unresolved endpoint
evidence, and emits config diagnostics for malformed declared
sections. `LocalDiscoveryConfig::from_env()` enables declared-link
loading by default while tests can inject or disable the config
loader explicitly.
<!-- SECTION:FINAL_SUMMARY:END -->

Legacy ID: `P5-003`
