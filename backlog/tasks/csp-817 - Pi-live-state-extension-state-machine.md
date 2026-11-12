---
id: CSP-817
title: 'Pi live state: extension state machine'
status: To Do
assignee: []
created_date: '2026-11-12 01:47'
updated_date: '2026-11-12 02:27'
labels:
  - live-state
  - ls-emit
milestone: m-26
dependencies:
  - CSP-810
documentation:
  - docs/plans/live-state/research/04-pi.md
  - >-
    docs/adr/0206-live-state-emitters-subscribe-to-a-minimal-event-set-per-harness.md
priority: medium
type: enhancement
ordinal: 903000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Pi already computes working, blocked, done, and error for its own terminal escape sequence, but tmux does not forward it, so Conspectus has to be told by its extension. Pi's extension handlers are awaited in its event loop, so the extension must keep state in memory and send snapshots sparingly.

Scope: Extend `plugins/pi-hook/conspectus-hook.ts` with the state machine of `04-pi.md` section 7 (events in ADR 0206, snapshot payload, immediate versus debounced emission, a 60 s unref'd heartbeat, shutdown reasons), a `pid` the Rust side trusts over the process walk, and the embedded copy and version marker so `hook status pi` reports `outdated` correctly. Rust side: the Pi snapshot mapping and fixtures (payload, event-script, tail). Classify errors inside the extension so no text leaves it. State the version floor (WAITING needs 0.84.4) in `hook status`.

TUI changes: none visible. The TUI renders the same frames before and after (check a few views with `conspectus tui --snapshot`).

Tests and checks: Event-script fixtures through a Rust mirror (tool run, parallel tools, permission gate during a tool, plan-mode prompt, retry then success, rate limit, compaction, abort, new session, reload, quit, kill); payload fixtures with unknown and legacy shapes; `hook status pi` outdated detection.

Blockers: The fast writer.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 The extension reports state on change and a heartbeat, never spawning a process on a streaming event
- [ ] #2 Error text never leaves the extension; only the class does
- [ ] #3 `hook status pi` reports an outdated extension and the version floor for WAITING
- [ ] #4 Fixtures and a Rust mirror verify the state machine without Node
<!-- AC:END -->

## Definition of Done
<!-- DOD:BEGIN -->
- [ ] #1 `just check` passes, or `git diff --check` for docs-only changes
- [ ] #2 Docs and ADRs are updated when behavior, architecture, or workflow change
<!-- DOD:END -->
