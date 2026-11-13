---
id: CSP-832
title: Live state from `claude agents --json`
status: In Progress
assignee:
  - '@claude'
created_date: '2026-11-12 01:48'
updated_date: '2026-11-13 05:20'
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
- [ ] #1 ADR 0218 records the check: claude agents --json starts no supervisor or other process, fires no hook, writes nothing to an initialized config, costs about 0.3 to 0.6 s per call, and reaches the network unless CLAUDE_CODE_DISABLE_NONESSENTIAL_TRAFFIC=1 is set, which Conspectus sets on the child
- [ ] #2 ADR 0218 is indexed in docs/adr/README.md, ADR 0207 and ADR 0028 point at it, and docs/capabilities.toml allows claude agents --json in the read context and SIGKILL to that child, citing 0218; tests/capabilities_static.rs and tests/capabilities.rs pass
- [ ] #3 A pure parser reads the output against a typed whitelist (unknown keys ignored; output over 1 MiB, malformed JSON, or a non-array asserts nothing) and maps it by the table in ADR 0218 (unknown status or state, or missing fields, assert nothing); fixture tests cover every shape under tests/fixtures/, labeled synthetic
- [ ] #4 The poll runs only from the daemon's discovery cycles and a daemonless TUI's background refresh, at most once per 30 s, and only while a bound Claude Code hook entry is working or waiting, has had no event for 10 s, and is not already answered by an earlier poll; one-shot commands, the TUI's UI thread, and the one-second overlay never run it; fake-runner and fake-clock tests cover the gate and the rate limit
- [ ] #5 A missing claude, a failure, a timeout, oversized or unparseable output asserts nothing, changes no binding, and prints nothing; a hung child is killed at the timeout (tested with a fake claude that never exits)
- [ ] #6 Carve-out tests: a hook event newer than the poll, or less than 10 s older, wins; a silent hook working plus an idle poll reads ready with source harness_poll; a silent hook waiting plus a busy poll reads working; hook ready, error, compacting, starting, unknown, and exited are never changed; a pid mismatch, an older poll, or a poll failure changes nothing; a session with no hook entry gets no poll state
- [ ] #7 End to end through discovery: a Claude Code hook entry stuck at working plus a fake poll saying idle makes the session read ready, and one stuck at waiting plus a busy poll makes it read working, and the TUI's overlay keeps the repair when it binds the store again
- [ ] #8 A sentinel planted in the name, cwd, unknown fields, unknown enum values, and a non-id session id of the poll output reaches neither the store, the snapshot bytes, the JSON output, Debug output, nor any error
- [ ] #9 CONSPECTUS_DISABLE_LIVE_POLL turns the poll off; graph.bin moves to format version 14 with its pinned tests; docs/operations.md, docs/design.md, docs/library-api.md, CHANGELOG.md, and the plan's as-built notes describe the source; the tui --snapshot frames of the default views are byte-identical
<!-- AC:END -->

## Definition of Done
<!-- DOD:BEGIN -->
- [ ] #1 `just check` passes, or `git diff --check` for docs-only changes
- [ ] #2 Docs and ADRs are updated when behavior, architecture, or workflow change
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
<!-- SECTION:NOTES:END -->
