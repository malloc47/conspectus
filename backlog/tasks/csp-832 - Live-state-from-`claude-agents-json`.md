---
id: CSP-832
title: Live state from `claude agents --json`
status: Done
assignee:
  - '@claude'
created_date: '2026-11-12 01:48'
updated_date: '2026-11-13 06:46'
labels:
  - live-state
  - ls-policy
milestone: m-26
dependencies:
  - CSP-812
documentation:
  - docs/adr/0207-live-state-sources-beyond-hooks-need-their-own-decision.md
  - docs/plans/live-state/research/01-claude-code.md
priority: low
type: enhancement
ordinal: 918000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Claude Code documents `claude agents --json` as the supported way to read session state from outside, with busy, waiting, and idle statuses and what a waiting session is waiting for. It closes the Esc-interrupt and approval-granted gaps but is a new subprocess with an unknown cost, and it may start a background supervisor.

Scope: First check that running it does not start a supervisor and measure its cost. Then an ADR, a `docs/capabilities.toml` entry (ADR 0117), an adapter that runs it only to repair stale `working` and `waiting` entries, and fixtures for the output shapes.

TUI changes: Fewer stuck `working` and `waiting` states.

Tests and checks: Fixture tests; the supervisor check recorded in the ADR.

Blockers: Operator decision 11.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [x] #1 ADR 0218 records the check: claude agents --json starts no supervisor or other process, fires no hook, writes nothing to an initialized config, costs about 0.3 to 0.6 s per call, and reaches the network unless CLAUDE_CODE_DISABLE_NONESSENTIAL_TRAFFIC=1 is set, which Conspectus sets on the child
- [x] #2 ADR 0218 is indexed in docs/adr/README.md, ADR 0207 and ADR 0028 point at it, and docs/capabilities.toml allows claude agents --json in the read context and SIGKILL to that child, citing 0218; tests/capabilities_static.rs and tests/capabilities.rs pass
- [x] #3 A pure parser reads the output against a typed whitelist (unknown keys ignored; output over 1 MiB, malformed JSON, or a non-array asserts nothing) and maps it by the table in ADR 0218 (unknown status or state, or missing fields, assert nothing); fixture tests cover every shape under tests/fixtures/, labeled synthetic
- [x] #4 The poll runs only from the daemon's discovery cycles and a daemonless TUI's background refresh, at most once per 30 s, and only while a bound Claude Code hook entry is working or waiting, has had no event for 10 s, and is not already answered by an earlier poll; one-shot commands, the TUI's UI thread, and the one-second overlay never run it; fake-runner and fake-clock tests cover the gate and the rate limit
- [x] #5 A missing claude, a failure, a timeout, oversized or unparseable output asserts nothing, changes no binding, and prints nothing; a hung child is killed at the timeout (tested with a fake claude that never exits)
- [x] #6 Carve-out tests: a hook event newer than the poll, or less than 10 s older, wins; a silent hook working plus an idle poll reads ready with source harness_poll; a silent hook waiting plus a busy poll reads working; hook ready, error, compacting, starting, unknown, and exited are never changed; a pid mismatch, an older poll, or a poll failure changes nothing; a session with no hook entry gets no poll state
- [x] #7 End to end through discovery: a Claude Code hook entry stuck at working plus a fake poll saying idle makes the session read ready, and one stuck at waiting plus a busy poll makes it read working, and the TUI's overlay keeps the repair when it binds the store again
- [x] #8 A sentinel planted in the name, cwd, unknown fields, unknown enum values, and a non-id session id of the poll output reaches neither the store, the snapshot bytes, the JSON output, Debug output, nor any error
- [x] #9 CONSPECTUS_DISABLE_LIVE_POLL turns the poll off; graph.bin moves to format version 14 with its pinned tests; docs/operations.md, docs/design.md, docs/library-api.md, CHANGELOG.md, and the plan's as-built notes describe the source; the tui --snapshot frames of the default views are byte-identical
<!-- AC:END -->

## Definition of Done
<!-- DOD:BEGIN -->
- [x] #1 `just check` passes, or `git diff --check` for docs-only changes
- [x] #2 Docs and ADRs are updated when behavior, architecture, or workflow change
<!-- DOD:END -->

## Implementation Plan

<!-- SECTION:PLAN:BEGIN -->
1. Check done by the coordinator, repeated in a scratch HOME/CLAUDE_CONFIG_DIR/XDG with env -i under strace: no supervisor, no child process, no hook fired, no write after first-start init, a TLS connection to Anthropic per run that CLAUDE_CODE_DISABLE_NONESSENTIAL_TRAFFIC=1 removes.
2. ADR 0218 (check results, cost, mapping table, repair gate, carve-out, hook-less sessions, privacy, switch, capabilities); index it; amend ADR 0207 and 0028 status; capabilities.toml read entry and SIGKILL.
3. Model: LiveSource::HarnessPoll (harness_poll), neutral LivePoll / PolledAgent, GraphSnapshot.live_polls (graph.bin only, not JSON); FORMAT_VERSION 14.
4. discovery/harness/claude_agents.rs: pure whitelist parser and the mapping; fixtures under tests/fixtures/claude-agents/ (synthetic).
5. discovery/live_poll.rs: PollRunner seam (system runner through exec::command, 3 s timeout, SIGKILL, 1 MiB cap, stderr dropped, nonessential traffic off), clock seam, rate limit and repair gate, remembered last poll with runs; LocalDiscoveryConfig.live_poll wired only into the daemon factory and the TUI's worker discover closure; CONSPECTUS_DISABLE_LIVE_POLL.
6. Fold the poll into bind_live after the titles (carve-out), so the discovery pass and the TUI overlay agree; detail-pane text for the new source.
7. Tests: parser and mapping tables, gate and rate limit, timeout with a hanging fake claude, carve-out matrix, discovery end to end, sentinel privacy, traced capabilities scenario with a claude shim.
8. Docs: operations Live agent state, design, library-api, CHANGELOG, plan as-built; CSP-822 note; frames byte-identical; checks and full tests.
<!-- SECTION:PLAN:END -->

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
Operator decision (2026-11-12), see docs/plans/live-state/README.md section 7.1 item 11: approved, after a check. First verify that running 'claude agents --json' does not start Claude's background supervisor and measure its cost per call, and record both in the ADR. If it starts the supervisor, stop and report instead of building the adapter.

Check repeated in a scratch HOME/CLAUDE_CONFIG_DIR/XDG with env -i under strace -f (the wrapped binary run directly, so the nix wrapper's token file was never read): one execve, no child process, no hook fired (scratch settings.json hooks touching marker files), supervisor control socket tried and absent, nothing written after the first-run init, 0.26 to 0.34 s per call (0.48 to 0.76 s under strace). New finding: every call opens a TLS connection to an Anthropic address on 443 unless CLAUDE_CODE_DISABLE_NONESSENTIAL_TRAFFIC=1 (DISABLE_TELEMETRY alone does not stop it); Conspectus sets it on the child. CLAUDE_CODE_DISABLE_AGENT_VIEW makes it exit 1.

Decisions taken where the brief left room: idle maps to ready with no detail (the list cannot tell finished from interrupted; the hook-side idle notification also maps to ready without detail); status decides, state stands in when status is absent or unknown; sandbox request reads permission, every other waitingFor input; a poll that read ready, error, or exited, or did not list the agent, answers the entry until its next event (an idle interrupted session costs one call), while working or waiting keep it due; an agreeing poll leaves the hook observation untouched; sessions with no hook entry get nothing (CSP-871 filed); one switch, CONSPECTUS_DISABLE_LIVE_POLL, for every poller; source name harness_poll; [signals] gained read = ["SIGKILL"].

Validation: checks.sh (fmt, clippy -D warnings, rustdoc -D warnings, git diff --check) exit 0; run-tests.sh 5619 passed, 1 failed (the known forge Text file busy flake, passed alone and with all 817 discovery tests); just tuikit-features exit 0; both intermediate code commits pass clippy on all targets and their narrow tests; the nine tui --snapshot frames are byte-identical to the CSP-831 tip.
<!-- SECTION:NOTES:END -->

## Final Summary

<!-- SECTION:FINAL_SUMMARY:BEGIN -->
Landed `claude agents --json` as a live-state repair for Claude Code (ADR 0218). The check the operator asked for is in the ADR: no supervisor, no child process, no hook fired, nothing written to an initialized config, 0.3 to 0.6 s per call, and a TLS connection per call unless `CLAUDE_CODE_DISABLE_NONESSENTIAL_TRAFFIC=1`, which Conspectus sets on the child.

- `src/discovery/harness/claude_code_agents.rs`: whitelist parser (pid, plain sessionId, status, state, waitingFor; 1 MiB and 256-session caps) and the mapping table; the `AgentList` runner through `exec::command`, 3 s timeout and SIGKILL, stderr never read. Synthetic fixtures in `tests/fixtures/claude-agents/`.
- `src/discovery/live_poller.rs`: `LivePoller` with a runner seam and a clock seam, at most one attempt per 30 s, only while a bound Claude Code hook entry recorded working or waiting, has been silent 10 s, and is not already answered; keeps the last successful poll. Built once by the daemon's discovery factory and the TUI's background refresh worker; one-shot commands, UI-thread refreshes, and the overlay never poll. `CONSPECTUS_DISABLE_LIVE_POLL` builds none.
- `src/discovery/live_poll.rs`: the fold in `bind_live` after the pane titles; a silent hook working or waiting reads the poll's state for the same process with source `harness_poll`; newer hook events win; ready, error, compacting, starting, unknown, exited untouched; hook-less sessions get nothing (CSP-871).
- Model: `LiveSource::HarnessPoll`, `LivePoll`, `PolledAgent`, `GraphSnapshot::live_polls` (graph.bin only); format version 14.
- Manifest: `[programs.harness] read = ["agents --json"]`, `[signals] read = ["SIGKILL"]`, citing 0218; the traced capability run now seeds a stuck entry and sees the daemon poll a stand-in `claude`.

Verified by unit tests (parser and mapping tables, gate and rate limit with fake runner and clock, timeout with a hanging stand-in, carve-out matrix, overlay rebinding), `tests/claude_agents_live.rs` (a daemon repairs an interrupted turn to ready and an answered approval to working, the switch and one-shot commands run nothing, a sentinel in the listing reaches no output, file, or stderr), checks.sh, the full suite (one known forge flake, passed on rerun), `just tuikit-features`, and byte-identical TUI frames. Docs: operations guide section, design, library API, CHANGELOG, plan as-built; CSP-822 notes what to record.
<!-- SECTION:FINAL_SUMMARY:END -->
