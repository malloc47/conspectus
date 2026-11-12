---
id: CSP-817
title: 'Pi live state: extension state machine'
status: To Do
assignee: []
created_date: '2026-11-12 01:47'
updated_date: '2026-11-12 11:22'
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
- [ ] #4 The state machine is a pure exported function tested with `node --test` against shared event-script fixtures that the Rust tests also consume
<!-- AC:END -->

## Definition of Done
<!-- DOD:BEGIN -->
- [ ] #1 `just check` passes, or `git diff --check` for docs-only changes
- [ ] #2 Docs and ADRs are updated when behavior, architecture, or workflow change
<!-- DOD:END -->

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
Planning note (after CSP-809): the extension is a snapshot source. It sends a `LiveSnapshot` (`src/live_store/`), which the store applies by `(instance, seq)`; the shared reducer is not involved. Read `LiveStore::apply_snapshot` and the `LiveSnapshot` type first.

Planning note (after CSP-810): the writer seam is on `HarnessAdapter` (`src/discovery/harness/mod.rs`): `live_events_from_payload(payload, context, observed_ms)` for event sources, `live_snapshot_from_payload` for plugin snapshots, `live_reducer` for a harness reducer, and `is_attribution_event`, which defaults to `true`. Override `is_attribution_event` so only the events that name the session (session start) take the spool and daemon path; every other event then reaches only the live-state store. `hook write` (`src/cli/hook_write.rs`) fills pid, pid start, and tmux pane/socket/session into the context; the adapter adds cwd and transcript path from its payload view. Claude Code's `LiveHook` view in `claude_code.rs` is the pattern (borrowed serde struct of whitelisted fields).

From CSP-813: `hook status` reports this harness's live-state health as `unsupported` until its adapter's `HarnessAdapter::reports_live_state` returns true; flip it when the hooks report live state, and update the `unsupported` row of the health table in `docs/operations.md`.

Verification approach (replaces the Rust mirror in the scope): keep the state machine a pure, exported function in `conspectus-hook.ts` with no side effects on import (the `ExtensionAPI` import is `import type`, so Node strips it) and test it with `node --test` (Node 24 strips types; `node` is in the user profile, not the dev shell). Shared event-script fixtures (JSONL: one Pi event per line, with the snapshot expected after it) are consumed by the TypeScript tests and by the Rust tests that feed the expected snapshots through the payload parser and the store. Add a `just` recipe for the plugin tests that reports "node not found" cleanly. ADR 0206 describes this.

From CSP-816: the opencode reducer's pattern is `plugins/opencode-hook/src/live.ts` (pure, no imports) with `test/live.test.ts` replaying `tests/fixtures/hooks/opencode-live/*.jsonl` (each step: an input, the payloads it must `emit`, and a hand-written `shown`); the Rust side replays the emitted payloads in `src/discovery/harness/opencode_live_sequence_tests.rs` with `live_sequences::assert_shown`. `just opencode-plugin-test` runs the opencode tests and fails with a message when node is missing; a Pi recipe can sit beside it or the two can share one.
<!-- SECTION:NOTES:END -->
