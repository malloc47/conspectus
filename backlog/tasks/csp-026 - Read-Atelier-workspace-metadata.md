---
id: CSP-026
title: Read Atelier workspace metadata
status: Done
assignee: []
created_date: '2026-05-15 03:58'
labels:
  - p2
milestone: m-3
dependencies:
  - CSP-023
  - CSP-025
ordinal: 26000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
- Scope: parse `atelier.toml` enough to emit Atelier workspace context,
  workspace repo membership evidence, and related source metadata without
  depending on Atelier command modules.
- Tests: fixture tests for minimal, multi-repo, and malformed Atelier
  workspace metadata.
- Manual checks: run from an Atelier workspace with no forks and inspect
  workspace, repo, checkout, and branch nodes.
- Blockers: `CSP-023`, `CSP-025`.
<!-- SECTION:DESCRIPTION:END -->

## Final Summary

<!-- SECTION:FINAL_SUMMARY:BEGIN -->
Added a read-only `atelier.toml` subset parser and parent-walk
workspace discovery that emits Atelier workspace nodes, discovered repo
graph fragments, and strong-discovered workspace membership links without
depending on Atelier command modules.
<!-- SECTION:FINAL_SUMMARY:END -->

Legacy ID: `P2-006`
