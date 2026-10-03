---
id: CSP-029
title: Wire local discovery into `graph --format json`
status: Done
assignee: []
created_date: '2026-05-15 03:58'
labels:
  - p2
milestone: m-3
dependencies:
  - CSP-024
  - CSP-028
ordinal: 29000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
- Scope: replace empty graph discovery with local discovery orchestration for
  cwd/configured roots while preserving deterministic output and existing
  Phase 1 JSON shape.
- Tests: CLI integration tests for plain repo, non-repo cwd, invalid scan
  roots, and deterministic output across repeated runs.
- Manual checks: run `cargo run -- graph --format json` from a plain repo, a
  linked worktree, an Atelier workspace with no forks, and an Atelier
  workspace with worktree, selected, and research forks.
- Blockers: `CSP-024`, `CSP-028`.
<!-- SECTION:DESCRIPTION:END -->

## Final Summary

<!-- SECTION:FINAL_SUMMARY:BEGIN -->
Wired `graph --format json` to local discovery from the current
directory or explicit `--scan-root` values, preserving deterministic JSON
output and adding CLI coverage for non-repo, plain repo, missing-root, and
invalid-format cases.
<!-- SECTION:FINAL_SUMMARY:END -->

Legacy ID: `P2-009`
