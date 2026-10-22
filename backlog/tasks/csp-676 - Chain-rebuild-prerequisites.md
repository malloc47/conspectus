---
id: CSP-676
title: Chain rebuild prerequisites
status: To Do
assignee: []
created_date: '2026-10-22 01:11'
labels:
  - history-pins
milestone: m-23
dependencies:
  - CSP-674
  - CSP-675
documentation:
  - docs/plans/history-rebuild-and-pin-slots.md
priority: medium
type: feature
ordinal: 734000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Some rebuilds need another rebuild first: a mux whose cwd was a removed worktree, or a worktree whose repo is gone. Chaining them in one plan replaces worktree-backed pins (ADR 0094), where a planned mux slot with a `worktree.branch` override realizes its worktree on first rebuild.

- Scope: prerequisite resolution across executors (mux → worktree → repo); one plan listing every step; stop and report at the first failed step; planned mux slots with a worktree branch build the worktree first.
- Tests: plan tests for each chain and a failure mid-chain.
- Manual checks: rebuild a ghost mux whose worktree was removed.
- Blockers: the worktree and clone executor tickets.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 A ghost mux on a gone worktree rebuilds the worktree, then the mux, in one plan
- [ ] #2 A worktree in a gone repo clones the repo first
- [ ] #3 A failed step stops the plan and reports which step failed and what was already done
<!-- AC:END -->

## Definition of Done
<!-- DOD:BEGIN -->
- [ ] #1 `just check` passes, or `git diff --check` for docs-only changes
- [ ] #2 Docs and ADRs are updated when behavior, architecture, or workflow change
<!-- DOD:END -->
