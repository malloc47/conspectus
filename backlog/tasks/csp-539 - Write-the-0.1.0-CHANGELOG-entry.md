---
id: CSP-539
title: Write the 0.1.0 CHANGELOG entry
status: Done
assignee: []
created_date: '2026-09-30 18:30'
labels:
  - rel
milestone: m-20
dependencies: []
priority: high
ordinal: 606000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
- Scope: `CHANGELOG.md` covers only Phase 11 (2026-06-23). Under
  `[Unreleased]`, write the first-release summary:
  - the interactive TUI (sessions and mux views, relationship explorer,
    controls, search, themes);
  - pins with resume continuity, and worktree-backed pins;
  - mux lifecycle (`mux new`, `mux launch`, teardown);
  - worktree list, new, rm, merge, close, and prune through worktrunk;
  - `conspectus serve` and the zero-copy snapshot;
  - the native transcript viewer;
  - `graph --format dot|html`;
  - `hook init|status|remove`;
  - zellij discovery and agent-deck workspaces;
  - the serve idle-cost work.

  Per decision 6, fold the Phase 11 removals into an "Upgrading from
  development builds" subsection. `CSP-541` stamps the version and date
  at tag time, so this story doesn't wait on the release decision.
- Tests: docs-only; `git diff --check`.
- Blockers: decision 6.
<!-- SECTION:DESCRIPTION:END -->

## Final Summary

<!-- SECTION:FINAL_SUMMARY:BEGIN -->
`[Unreleased]` now describes the 0.1.0 feature set; per the
operator's call the Phase 11 development-build notes were dropped
(the upgrade notes remain in `docs/operations.md`). `CSP-541` stamps
the version and date.
<!-- SECTION:FINAL_SUMMARY:END -->

Legacy ID: `REL-008`
