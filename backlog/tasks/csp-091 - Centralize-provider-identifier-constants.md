---
id: CSP-091
title: Centralize provider identifier constants
status: Done
assignee: []
created_date: '2026-05-17 20:45'
labels:
  - h-ref
milestone: m-11
dependencies: []
ordinal: 91000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
- Landed 2026-07-05 (`5b58aaf`). Migrated all major
  provider-key string literals to the `providers::*`
  constants defined by CSP-473:
  * `discovery/mod.rs::discover_local_warm_with` uses
    `providers::GIT`, `ATELIER`, `GENERIC_WORKSPACE`,
    `CLAUDE_CODE`/`CODEX`/`OPENCODE`/`AIDER`, `TMUX`,
    `ZELLIJ`, `GITHUB`.
  * `discovery/orchestrator::REGISTRY` agent_deck entry
    uses `providers::AGENT_DECK`.
  * `discovery/atelier.rs` producer-side literals use
    `providers::ATELIER`.
  * Added `providers::GITLAB` const;
    `forge/gitlab.rs::GITLAB_PROVIDER` points at it.
- Blockers: none.
<!-- SECTION:DESCRIPTION:END -->

Legacy ID: `H-REF-009`
