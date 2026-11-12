---
id: CSP-811
title: 'Claude Code live state: reducer, fixtures, and hook install'
status: To Do
assignee: []
created_date: '2026-11-12 01:47'
labels:
  - live-state
  - ls-emit
milestone: m-26
dependencies:
  - CSP-810
documentation:
  - docs/plans/live-state/research/01-claude-code.md
  - >-
    docs/adr/0205-live-state-emitters-subscribe-to-a-minimal-event-set-per-harness.md
priority: high
type: enhancement
ordinal: 897000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Claude Code is the harness the operator uses most and has the richest hook set, so it is the vertical slice that proves the design end to end. Its installed hook covers session start only, with a matcher that misses the newer `fork` source, so nothing reports permission waits, stops, or subagents.

Scope: Implement the Claude Code adapter's `LiveEvent` mapping and reducer from the research (`01-claude-code.md` section 6): the 14 events of ADR 0205, per-agent waits, the subagent set, compaction, `StopFailure` types, `SessionEnd`, and the Stop-time reconcile of background subagents. Extend `hook init claude-code` to install the full set idempotently (upgrading an older install), with a 5 s timeout on every entry, the typed `Notification` matcher, the complete `SessionStart` matcher, and exec-form detection; extend `hook status claude-code` to show per-event coverage; make `hook remove` remove every entry it added and nothing else. Build the fixtures from documented payload shapes: single payloads and ordered sequences for the reducer (simple turn, permission approve, deny, Esc interrupt, AskUserQuestion, plan approve, parallel and background subagents, auto-compact, `/clear`, rate limit, kill -9). Replace the existing fixtures that use the invalid `source: "user"`. Record the known gaps in the reducer's documentation.

TUI changes: none visible. The TUI renders the same frames before and after (check a few views with `conspectus tui --snapshot`).

Tests and checks: Reducer sequence tests asserting the effective state after each event; install, upgrade, status, and remove tests in a scratch home that never touch the real `~/.claude`; the operator's own hooks and unrelated keys survive; sentinel privacy tests; fixtures are labeled synthetic until the verification story re-records them.

Blockers: The fast writer.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 The reducer maps the 14 subscribed events to the states in the research table, with a sequence test per scenario listed in the scope
- [ ] #2 `hook init claude-code` installs the full set with timeouts, is idempotent, upgrades a session-start-only install, and `hook remove` removes only its entries
- [ ] #3 `hook status claude-code` reports which events are installed
- [ ] #4 The `fork` source is matched and the invalid-source fixtures are replaced
- [ ] #5 No payload text reaches the store (sentinel test)
<!-- AC:END -->

## Definition of Done
<!-- DOD:BEGIN -->
- [ ] #1 `just check` passes, or `git diff --check` for docs-only changes
- [ ] #2 Docs and ADRs are updated when behavior, architecture, or workflow change
<!-- DOD:END -->
