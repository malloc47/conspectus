---
id: CSP-034
title: Discover supported agent sessions
status: Done
assignee: []
created_date: '2026-05-15 20:50'
labels:
  - p3
milestone: m-4
dependencies:
  - CSP-032
  - CSP-033
ordinal: 34000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
- Scope: implement read-only adapters that emit Conspectus-native
  `AgentSession` nodes and source metadata for supported local state from
  `claude-code`, `opencode`, `codex`, and `aider`.
- Tests: fixture-based adapter tests for discovered sessions, orphaned
  sessions, malformed records, missing optional fields, and stable node IDs.
- Manual checks: run against local synthetic state roots and inspect session
  nodes for readable provider metadata.
- Blockers: `CSP-032`, `CSP-033`.
<!-- SECTION:DESCRIPTION:END -->

## Final Summary

<!-- SECTION:FINAL_SUMMARY:BEGIN -->
Added `CodexAdapter`, `ClaudeCodeAdapter`, `OpenCodeAdapter`, and
`AiderAdapter`; widened the `HarnessAdapter` trait to receive the full
`DiscoveryContext` so the per-repo aider adapter can walk scan roots while
state-root harnesses pull their root via `harness_state_root`. Covered
discovered sessions, missing state directories, malformed records, missing
optional fields, and stable ID reproducibility for each adapter.
<!-- SECTION:FINAL_SUMMARY:END -->

Legacy ID: `P3-003`
