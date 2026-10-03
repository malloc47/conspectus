---
id: CSP-522
title: '`close-down` compound orchestrator (ADR 0093)'
status: Done
assignee: []
created_date: '2026-08-05 02:51'
labels:
  - h-wt
milestone: m-18
dependencies: []
ordinal: 560000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Landed in layers: (1) `MuxBackend::kill_session` primitive + `tmux::teardown` two-phase graceful(`SIGTERM`)→grace→hard mechanism behind a `ProcessSignaller` seam + `[worktree] teardown_confirm`/`teardown_grace` config; (2) a shared, provider-neutral `discovery::worktree::close_down` module (`plan_close_down` graph query + `execute_close_down` = terminate mux sessions → merge|remove → best-effort pin-drop, returning a `CloseDownReport`), unit-tested with fakes; (3) CLI `worktree close <branch> --merge|--discard [--target] [--yes] [--grace]`; (4) TUI close-down — `CloseDownWorktree` action + hot key `X` opening a merge/discard choice → `Msg::CommitWorktreeCloseDown` → `StoreOp::WorktreeCloseDown` → `execute_worktree_close_down`. Confirmation honors the policy (`should_confirm`). Full suite green (2010).
<!-- SECTION:DESCRIPTION:END -->

Legacy ID: `H-WT-006`
