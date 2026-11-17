---
id: CSP-888
title: Keep the real-session recording driver in the repository as a manual dev tool
status: Done
assignee:
  - '@claude'
created_date: '2026-11-16 18:01'
updated_date: '2026-11-17 01:02'
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
- [x] #1 The driver (private tmux server, clean per-harness environment that keeps TMUX and TMUX_PANE, scratch Claude Code, Codex, and Pi configuration copies, a scratch XDG_STATE_HOME, screen and title snapshots, a timeline log, and a sampler of title and stored state) lives under scripts/ with a README, and nothing in it runs in CI or from `just check`
- [x] #2 One scenario script per harness reproduces the checks CSP-822 ran (Claude Code: launch, permission prompt, approval during a long tool, Esc interrupt, question, subagent, compaction, clear, kill; Codex and Pi: their equivalents), each ending with the trace and a screen snapshot saved under a named directory
- [x] #3 Every run is memory-capped in its own systemd scope with the OOM policy set to continue, never gives a pty tool an unbounded input, and removes its scratch directories on exit; the README says why
- [x] #4 A `just` recipe starts a scenario and prints what quota it will use and which directories it creates, and an ADR (or an amendment to ADR 0242) records the tool
<!-- AC:END -->

## Definition of Done
<!-- DOD:BEGIN -->
- [x] #1 `just check` passes, or `git diff --check` for docs-only changes
- [x] #2 Docs and ADRs are updated when behavior, architecture, or workflow change
<!-- DOD:END -->

## Final Summary

<!-- SECTION:FINAL_SUMMARY:BEGIN -->
`scripts/record/` is the real-session recording driver, started by `just record HARNESS` (claude-code, codex, or pi): a private tmux server, a clean environment per harness that keeps only the private server's TMUX and TMUX_PANE, scratch copies of the harness's configuration with the hooks pointed at the chosen `conspectus` (so the installed link is never swapped), a scratch XDG_STATE_HOME, a sampler of title and stored state, screen snapshots, a timeline, and the redacted traces with a summary table, kept in the git-ignored `record-out/`. One scenario per harness reproduces what CSP-822 ran. It is manual: `--dry-run` prints the quota and the directories, a run needs `--yes` or the word `record` typed at a terminal, and it refuses without a terminal; each run is in its own `systemd-run --user --scope` with a memory cap, no swap, `OOMPolicy=continue`, and a wall-clock limit, never gives a pty tool an unbounded input, forwards a signal to the scenario, ends every process it started (found by a marker in their environment), and removes its scratch directory. ADR 0244 records the decision; `tests/record_driver.rs` checks the scripts parse, none reads /dev/zero or runs `yes` or `script`, the recipe is in no check or workflow, a dry run creates nothing, and an unconfirmed run refuses before creating anything. Verified by running each scenario end to end against the real Pi 1.5.2, Claude Code 2.1.322, and Codex 0.167.0 (which found and fixed a Claude permission mode that skipped the dialogs, a Codex review dialog that arrives after the first screen and was disabling a hook, a Codex service tier that needs setting per session, and a signal that did not reach the scenario). Gate: as for CSP-891.
<!-- SECTION:FINAL_SUMMARY:END -->
