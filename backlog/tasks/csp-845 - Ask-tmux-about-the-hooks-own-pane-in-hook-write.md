---
id: CSP-845
title: Ask tmux about the hook's own pane in hook write
status: Done
assignee:
  - '@claude'
created_date: '2026-11-12 10:20'
updated_date: '2026-11-12 13:12'
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
- [x] #1 When TMUX_PANE is set, hook write asks tmux about that pane (display-message -t), for the session name and the pane id
- [x] #2 Without TMUX_PANE the writer behaves as today
- [x] #3 A test with a stand-in tmux records the arguments and checks the target
<!-- AC:END -->

## Definition of Done
<!-- DOD:BEGIN -->
- [x] #1 `just check` passes, or `git diff --check` for docs-only changes
- [x] #2 Docs and ADRs are updated when behavior, architecture, or workflow change
<!-- DOD:END -->

## Implementation Plan

<!-- SECTION:PLAN:BEGIN -->
1. SystemTmux::current_session_context reads TMUX_PANE and, when it is a pane id (% and digits), passes `-t <pane>` to both display-message calls (session name, pane id); otherwise the calls stay untargeted as before. Split the env read from the probe (session_context_of) and the argument building (display_message_args) so they test without touching the environment. Both callers use it: hook write (attribution record and the live-state entry's first-sight session name, which share one answer) and the TUI's self-attach guard.
2. A pane id tmux does not answer for gives no session or pane (the record keeps only the socket); it does not fall back to the untargeted question, which would name another pane.
3. Tests: unit tests for the arguments and the pane-id check; tests/hook_write.rs with the stand-in tmux recording the arguments (`-t %7` with TMUX_PANE, none without or with a non-pane value); a private tmux server (own -S socket, TMUX removed, killed on drop) with two panes of one session, a writer per pane, asserting each record's session and pane.
4. Docs: module and trait doc comments, ADR 0028 amendment line, operations.md, the Codex research's open check answered, CHANGELOG.
<!-- SECTION:PLAN:END -->

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
From CSP-816: the opencode plugin now spawns `conspectus hook write opencode` detached (`detached: true`, a new session with no controlling terminal), so a 1.x attribution record's tmux session and pane may depend on this too. The live-state entry takes its pane from `TMUX_PANE` either way.

Verified the bug on a private tmux server (own -S socket, TMUX unset, killed after): with no terminal, TMUX set to that socket and TMUX_PANE=%0, an untargeted `display-message -p '#{pane_id}'` answered the active pane %1 on tmux 3.6a (dev shell) and 3.7c; `-t %0` answered `work %0`; `-t %9` (no such pane) printed nothing with exit 0.
Decision: a well-formed TMUX_PANE that tmux does not answer for gives no session or pane (the record keeps only the socket) instead of falling back to the untargeted query, which would name another pane. Unset, empty, or non-pane values (`not-a-pane`, `work:0.1`) keep the untargeted query.
Both callers use the probe: hook write (attribution record and the live-state entry's first-sight session name share one answer) and the TUI's self-attach guard (`cli/tui.rs` current_tmux_session_name), which had a terminal and so already got its own pane.
Validation: run-tests.sh 5368 passed, 0 failed; checks.sh pass; 9 tui --snapshot frames byte-identical.
<!-- SECTION:NOTES:END -->

## Final Summary

<!-- SECTION:FINAL_SUMMARY:BEGIN -->
`hook write` now asks tmux about its own pane. When `TMUX_PANE` is a pane id, `SystemTmux::current_session_context` passes `-t <pane>` to both `display-message` queries (session name, pane id), so the attribution record's session and pane, and the live-state entry's first-sight session name, come from the hook's pane. Unset, empty, or non-pane values keep today's untargeted query. A pane id tmux doesn't know gets no answer rather than another pane's. The TUI's self-attach guard uses the same probe.

The bug is real. On a private tmux server (tmux 3.6a and 3.7c), a query from a process with no terminal and `TMUX_PANE=%0` answered the active pane `%1`.

Verified:
- Unit tests in `tmux_tests.rs` cover the arguments and the pane-id check.
- `tests/hook_write.rs`: the stand-in tmux records `display-message -p -t %7 ...` for both queries. Without `TMUX_PANE`, and with an empty or non-pane value, the queries stay untargeted. A real private tmux server (its own `-S` socket, `TMUX` removed, killed on drop) with two panes of one session gives each pane's writer its own spool record and live-state entry with the right session and pane, while the other pane is active.
- `run-tests.sh`: 5368 passed, 0 failed. `checks.sh` passes. The nine `tui --snapshot` frames are byte-identical. nextest and `just tuikit-features` were not rerun because tuikit is untouched.

Docs: an ADR 0028 amendment line, operations.md, the Codex research's open check answered, and a CHANGELOG entry.
<!-- SECTION:FINAL_SUMMARY:END -->
