---
id: CSP-870
title: Widen the pane-title source to non-active panes and wrapped Codex launches
status: To Do
assignee: []
created_date: '2026-11-13 04:56'
labels:
  - live-state
  - ls-policy
milestone: m-26
dependencies:
  - CSP-831
priority: low
ordinal: 950000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
The pane-title source (ADR 0217) reads only each tmux session's active pane, from the existing `list-sessions` call, and only panes whose program tmux reports as `codex`. A Codex in another pane of a split window or in a window that is not current gives it nothing, and a Codex started from an npm install shows as `node`. Covering them needs either a second tmux call per cycle (`list-panes -a`) or a way to recognize a wrapped Codex without guessing, so it waits until it is worth the cost.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 A decision says whether the probe lists every pane's title (and at what cost per cycle) or stays with active panes, recorded against ADR 0217
- [ ] #2 If widened, a Codex in a non-active pane shows its title's state on its pane, with tests over a multi-pane listing
- [ ] #3 Codex launched through a wrapper is recognized only on evidence the hook sidecar would also accept
<!-- AC:END -->

## Definition of Done
<!-- DOD:BEGIN -->
- [ ] #1 `just check` passes, or `git diff --check` for docs-only changes
- [ ] #2 Docs and ADRs are updated when behavior, architecture, or workflow change
<!-- DOD:END -->
