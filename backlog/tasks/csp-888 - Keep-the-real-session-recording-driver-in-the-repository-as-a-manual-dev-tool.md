---
id: CSP-888
title: Keep the real-session recording driver in the repository as a manual dev tool
status: To Do
assignee: []
created_date: '2026-11-16 18:01'
updated_date: '2026-11-16 20:02'
labels:
  - live-state
  - ls-emit
milestone: m-26
dependencies:
  - CSP-887
priority: medium
type: task
ordinal: 964000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
CSP-822 tied off live state against real sessions of Claude Code, Codex, and Pi by driving each harness in a private tmux server, in a throwaway directory, with a clean environment, scratch copies of the harness configuration, and a scratch state directory, while the hook trace (ADR 0242, CSP-887) recorded what `conspectus hook write` received. That driver was written as throwaway shell in RAM-backed /tmp. The recording is only worth repeating if the driver is too: harnesses change often, and the reducers rest on assumptions that only a real session can confirm again.

The driver must stay manual. It spends the operator's model quota and uses their accounts, so it never runs in CI, and it must never start by accident. Two hazards found while building it belong in the tool, not in someone's memory: a pty tool fed from an endless source (`script` reading `/dev/zero`) grew to 7-9 GB in seconds and, with swap full and `OOMPolicy=stop` on tmux pane scopes, took down the whole session twice; and a harness started from inside another harness inherits its session variables (`CLAUDE_PID`, `CLAUDECODE`, `TMUX`), which corrupts what the hooks see.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 The driver (private tmux server, clean per-harness environment that keeps TMUX and TMUX_PANE, scratch Claude Code, Codex, and Pi configuration copies, a scratch XDG_STATE_HOME, screen and title snapshots, a timeline log, and a sampler of title and stored state) lives under scripts/ with a README, and nothing in it runs in CI or from `just check`
- [ ] #2 One scenario script per harness reproduces the checks CSP-822 ran (Claude Code: launch, permission prompt, approval during a long tool, Esc interrupt, question, subagent, compaction, clear, kill; Codex and Pi: their equivalents), each ending with the trace and a screen snapshot saved under a named directory
- [ ] #3 Every run is memory-capped in its own systemd scope with the OOM policy set to continue, never gives a pty tool an unbounded input, and removes its scratch directories on exit; the README says why
- [ ] #4 A `just` recipe starts a scenario and prints what quota it will use and which directories it creates, and an ADR (or an amendment to ADR 0242) records the tool
<!-- AC:END -->

## Definition of Done
<!-- DOD:BEGIN -->
- [ ] #1 `just check` passes, or `git diff --check` for docs-only changes
- [ ] #2 Docs and ADRs are updated when behavior, architecture, or workflow change
<!-- DOD:END -->
