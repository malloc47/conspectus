---
id: CSP-889
title: >-
  Replay a recorded hook trace through the adapters and compare it with the
  recorded states
status: To Do
assignee: []
created_date: '2026-11-16 18:02'
updated_date: '2026-11-16 20:02'
labels:
  - live-state
  - ls-emit
milestone: m-26
dependencies:
  - CSP-887
priority: medium
type: enhancement
ordinal: 965000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
The hook trace (ADR 0242, CSP-887) keeps exactly the fields the adapters read (enum values, tool classification, ids as stable pseudonyms) plus the neutral events the adapter produced and the state the reducer computed after each line. That makes a trace replayable: feeding each recorded payload back through the current adapter and reducer in a scratch store should reproduce the recorded events and states. Doing it as a test turns every committed recording into a regression test that fails when a harness change, an adapter edit, or a reducer edit contradicts what a real session produced. It also tests the redaction itself: if a redacted payload no longer maps to the same events, the redaction removed something an adapter needs.

The synthetic sequence fixtures from the harness stories (built from documentation) are what this replaces where a real recording differs. The first recordings come from CSP-822.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 A test replays every trace under tests/fixtures/hooks/recorded/ (one directory per harness and version) line by line through the harness adapter and reducer in a scratch store, and fails when the produced events or the stored state differ from the recorded ones, naming the line
- [ ] #2 The CSP-822 recordings for Claude Code, Codex, and Pi are committed there with the harness version in their names, and each replaces the synthetic sequence it contradicted
- [ ] #3 Changing a mapping in an adapter or a transition in the reducer makes the test fail (shown once with a deliberate edit)
- [ ] #4 The docs say how to record, redact-check, and add a trace for a new harness version, and how to read a replay failure
<!-- AC:END -->

## Definition of Done
<!-- DOD:BEGIN -->
- [ ] #1 `just check` passes, or `git diff --check` for docs-only changes
- [ ] #2 Docs and ADRs are updated when behavior, architecture, or workflow change
<!-- DOD:END -->
