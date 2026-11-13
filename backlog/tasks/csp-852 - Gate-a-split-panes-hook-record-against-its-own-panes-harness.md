---
id: CSP-852
title: Gate a split pane's hook record against its own pane's harness
status: To Do
assignee: []
created_date: '2026-11-12 12:52'
labels:
  - live-state
  - ls-fix
milestone: m-26
dependencies: []
documentation:
  - docs/plans/live-state/research/06-internal-audit.md
priority: low
type: bug
ordinal: 937000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Attribution ignores a hook record when the mux's *active* pane runs a different harness (`pane_running_harness` in `src/discovery/hook_sidecar.rs` reads `MuxSessionNode.active_pane_command`). Since the spool keeps one record per pane (CSP-818), two agents of different harnesses in split panes of one tmux session both keep records, but whichever pane is not active is marked Ignored ("pane now running ...") against the other pane's command, so only one of them is attributed. The graph has no pane entity and the mux node describes only the active pane, so the gate cannot see the record's own pane. Found while fixing the spool keys (CSP-818); see the internal audit section 1.8.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 A hook record from a non-active pane is not ignored because the active pane runs another harness
- [ ] #2 A record whose own pane now runs another harness is still ignored
- [ ] #3 Two agents of different harnesses in split panes of one tmux session are both attributed after a refresh
<!-- AC:END -->

## Definition of Done
<!-- DOD:BEGIN -->
- [ ] #1 `just check` passes, or `git diff --check` for docs-only changes
- [ ] #2 Docs and ADRs are updated when behavior, architecture, or workflow change
<!-- DOD:END -->
