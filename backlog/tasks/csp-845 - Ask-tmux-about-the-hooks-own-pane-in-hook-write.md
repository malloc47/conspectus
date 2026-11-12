---
id: CSP-845
title: Ask tmux about the hook's own pane in hook write
status: To Do
assignee: []
created_date: '2026-11-12 10:20'
labels:
  - live-state
  - ls-fix
milestone: m-26
dependencies: []
priority: low
type: bug
ordinal: 930000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
`SystemTmux::current_session_context` runs `tmux display-message -p '#{session_name}'` and `'#{pane_id}'` without `-t "$TMUX_PANE"`, so tmux answers for whatever it considers current. Since 0.155, Codex detaches hook commands from the controlling terminal, and the Codex research (`docs/plans/live-state/research/02-codex.md` section 4.4, UNVERIFIED) expects a detached writer in a multi-pane session to be told about the session's active pane, not its own. The live-state entry already takes its pane from `TMUX_PANE`, but its tmux session name and the attribution record's session and pane still come from that answer, so a split-pane Codex could be attributed to the wrong pane or session. Found while building the Codex slice (CSP-815). The attribution spool's pane keying is CSP-818; this is the question the writer asks tmux.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 When TMUX_PANE is set, hook write asks tmux about that pane (display-message -t), for the session name and the pane id
- [ ] #2 Without TMUX_PANE the writer behaves as today
- [ ] #3 A test with a stand-in tmux records the arguments and checks the target
<!-- AC:END -->

## Definition of Done
<!-- DOD:BEGIN -->
- [ ] #1 `just check` passes, or `git diff --check` for docs-only changes
- [ ] #2 Docs and ADRs are updated when behavior, architecture, or workflow change
<!-- DOD:END -->
