---
id: CSP-816
title: 'OpenCode live state: plugin reducers for 1.x and 2.x'
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
  - docs/plans/live-state/research/03-opencode.md
  - >-
    docs/adr/0205-live-state-emitters-subscribe-to-a-minimal-event-set-per-harness.md
priority: medium
type: enhancement
ordinal: 902000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
The OpenCode plugin forwards the event name only, which cannot tell busy from idle, and keeping state needs a reducer inside the plugin because OpenCode publishes permission asks and questions as events that no storage poll can see. 2.x also needs a server entrypoint so one writer sees every session, and the research found two plugin defects.

Scope: Extend the 1.x plugin with a per-session reducer that sends a complete snapshot (`phase`, pending permission/question/form counts, outcome, retry, parent) on change plus a 60 s heartbeat while not idle; add the 2.x server entrypoint (amending ADR 0126) listening to the events of ADR 0205; raise a child session's wait to its root; fix the 1.x forwarding of child-session events to the attribution path and the dead `session.compacted` filter entry in 2.x. Rust side: the OpenCode `LiveEvent` snapshot mapping with the whitelist and fixtures. Since the dev shell has no Node, verify the reducer through the event-script fixtures replayed by a Rust mirror in tests, and record the manual checks.

TUI changes: none visible. The TUI renders the same frames before and after (check a few views with `conspectus tui --snapshot`).

Tests and checks: Payload fixtures (busy, retry, permission, question, error, child, legacy), event-script fixtures with expected snapshots, a privacy test that no `resources`, `patterns`, or message survives, child-to-root rollup, `seq` ordering within one second, and the plugin build still reproducible (committed output).

Blockers: The fast writer.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 The plugin sends a state snapshot on change and a heartbeat while not idle, with `instance` and `seq`
- [ ] #2 A child session's wait raises `waiting` with `via_subagent` on its root session
- [ ] #3 The two plugin defects from the research are fixed
- [ ] #4 Fixtures and a Rust mirror verify the reducer without Node, and the manual checks are recorded
<!-- AC:END -->

## Definition of Done
<!-- DOD:BEGIN -->
- [ ] #1 `just check` passes, or `git diff --check` for docs-only changes
- [ ] #2 Docs and ADRs are updated when behavior, architecture, or workflow change
<!-- DOD:END -->
