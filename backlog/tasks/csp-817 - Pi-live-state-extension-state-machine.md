---
id: CSP-817
title: 'Pi live state: extension state machine'
status: Done
assignee:
  - '@claude'
created_date: '2026-11-12 01:47'
updated_date: '2026-11-12 12:12'
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
- [x] #1 The extension reports state on change and a heartbeat, never spawning a process on a streaming event
- [x] #2 Error text never leaves the extension; only the class does
- [x] #3 `hook status pi` reports an outdated extension and the version floor for WAITING
- [x] #4 The state machine is a pure exported function tested with `node --test` against shared event-script fixtures that the Rust tests also consume
<!-- AC:END -->

## Definition of Done
<!-- DOD:BEGIN -->
- [x] #1 `just check` passes, or `git diff --check` for docs-only changes
- [x] #2 Docs and ADRs are updated when behavior, architecture, or workflow change
<!-- DOD:END -->

## Implementation Plan

<!-- SECTION:PLAN:BEGIN -->
1. Extension (plugins/pi-hook/conspectus-hook.ts, one file): a pure state machine (run, compacting, prompt, top-level tools with the subagent set, last outcome, error class classified in the extension, retrying, rate limit) deriving the ADR 0203 state; immediate sends for waiting/ready/error/exited/compacting changes, a 500 ms coalescing window between working sub-states, a 60 s heartbeat while not ready; payload {session_id, cwd, transcript_path, hook_event_name, live{instance, seq, observed_ms, heartbeat_s, pid, pid_start, state, detail, subagents, since_ms}}. Default export stays the entry; the machine is reachable through one named export documented as test-only. EXTENSION_VERSION 2. Custom ui prompts are not waiting.
2. Shared event scripts tests/fixtures/hooks/pi-live/*.jsonl (synthetic, the research 7.5 scenarios) plus payload-*.json; node --test tests in plugins/pi-hook/test/ replaying every script and running the glue under node with a stand-in writer; just pi-plugin-test.
3. Rust: share the plugin snapshot parser between opencode and pi; Pi adapter is_attribution_event (session_start:* and legacy payloads), live_snapshot_from_payload, reports_live_state, the attribution record trusts the payload pid; sequence tests through the store (ordering, heartbeat/quiet, kill), parser tests, sentinel tests in tests/hook_write.rs.
4. hook status pi: says when the installed copy is older and whether it reports live state, and the version floor (waiting needs Pi 0.84.4, 1.1.0 recommended).
5. Docs: Pi hook README, operations.md Pi live state, ADR 0206 Pi as built, ADR 0121 amendment, ADR index, design.md, CHANGELOG, plan README.
6. Checks (checks.sh, run-tests.sh, node tests, frames byte-identical), finalize.
<!-- SECTION:PLAN:END -->

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
Planning note (after CSP-809): the extension is a snapshot source. It sends a `LiveSnapshot` (`src/live_store/`), which the store applies by `(instance, seq)`; the shared reducer is not involved. Read `LiveStore::apply_snapshot` and the `LiveSnapshot` type first.

Planning note (after CSP-810): the writer seam is on `HarnessAdapter` (`src/discovery/harness/mod.rs`): `live_events_from_payload(payload, context, observed_ms)` for event sources, `live_snapshot_from_payload` for plugin snapshots, `live_reducer` for a harness reducer, and `is_attribution_event`, which defaults to `true`. Override `is_attribution_event` so only the events that name the session (session start) take the spool and daemon path; every other event then reaches only the live-state store. `hook write` (`src/cli/hook_write.rs`) fills pid, pid start, and tmux pane/socket/session into the context; the adapter adds cwd and transcript path from its payload view. Claude Code's `LiveHook` view in `claude_code.rs` is the pattern (borrowed serde struct of whitelisted fields).

From CSP-813: `hook status` reports this harness's live-state health as `unsupported` until its adapter's `HarnessAdapter::reports_live_state` returns true; flip it when the hooks report live state, and update the `unsupported` row of the health table in `docs/operations.md`.

Verification approach (replaces the Rust mirror in the scope): keep the state machine a pure, exported function in `conspectus-hook.ts` with no side effects on import (the `ExtensionAPI` import is `import type`, so Node strips it) and test it with `node --test` (Node 24 strips types; `node` is in the user profile, not the dev shell). Shared event-script fixtures (JSONL: one Pi event per line, with the snapshot expected after it) are consumed by the TypeScript tests and by the Rust tests that feed the expected snapshots through the payload parser and the store. Add a `just` recipe for the plugin tests that reports "node not found" cleanly. ADR 0206 describes this.

From CSP-816: the opencode reducer's pattern is `plugins/opencode-hook/src/live.ts` (pure, no imports) with `test/live.test.ts` replaying `tests/fixtures/hooks/opencode-live/*.jsonl` (each step: an input, the payloads it must `emit`, and a hand-written `shown`); the Rust side replays the emitted payloads in `src/discovery/harness/opencode_live_sequence_tests.rs` with `live_sequences::assert_shown`. `just opencode-plugin-test` runs the opencode tests and fails with a message when node is missing; a Pi recipe can sit beside it or the two can share one.

Built as planned, in four commits (extension and node tests, shared parser refactor, Pi adapter, hook status) plus docs.

Choices made where the story or research left one (all recorded in ADR 0206 decisions 25 to 28):
- The machine lives in `plugins/pi-hook/conspectus-hook.ts` and is reachable only through one named export documented as test-only (`testOnly`): no source confirmed what Pi does with exports besides the default.
- `ui_prompt_start{kind: custom}` (and unknown kinds) change nothing, so loaders and widgets raise no false waits. confirm is permission; select, input, editor are question.
- A change between working sub-states is held 500 ms from the first such change (not restarted), the latest sent; any immediate change drops it. The heartbeat is 60 s after the last snapshot while not ready (working, waiting, compacting, error).
- quit, new, resume, fork: exited/ended for the session left (the vocabulary has no detached state). reload: nothing from the old load; the new load (new instance) reports afresh.
- Errors are classified in the extension from the request's latest error status, else the message words; rate limit, auth, overloaded keep their detail, the rest read failed. `after_provider_response` is subscribed for its status only.
- The extension's pid replaces the writer's parent walk for the attribution record too (research 7.4); a live check (CSP-630, CSP-822) still has to confirm it.
- `hook status pi` reads the installed copy's EXTENSION_VERSION; a copy older than 2 reports no live state, so the health line says off.
- Not done: tail fixtures (the hook-free fallback is CSP-833; the mtime fix is CSP-820), the Pi version in snapshots (CSP-851, needs a live Pi), a configurable subagent tool set (CSP-850).

Validation: `just pi-plugin-test` 31/31 on Node 24.21 (TypeScript executed under node with type stripping; nothing compiled against Pi's types); `just opencode-plugin-test` still passes; Rust Pi tests (parser, 18 replayed scripts, sentinels in hook_write.rs, hook status in cli_smoke.rs); checks.sh (fmt, clippy -D warnings, rustdoc -D warnings, diff --check); full suite via run-tests.sh; nine `tui --snapshot` frames byte-identical to the baseline.
<!-- SECTION:NOTES:END -->

## Final Summary

<!-- SECTION:FINAL_SUMMARY:BEGIN -->
Landed Pi live state. `plugins/pi-hook/conspectus-hook.ts` (EXTENSION_VERSION 2) keeps a pure state machine (run, compaction, open dialog, top-level tool calls with the `subagent` tool counted as subagents, last outcome, an error class it computes itself, retrying, a 429). It sends the whole snapshot to `conspectus hook write pi`: at once for waiting, ready, error, exited, and compaction changes, after a 500 ms window for working sub-state changes, and every 60 s while not ready. It subscribes to no streaming event, waits on nothing, and sends no text. Custom dialogs are not a wait. The machine is reachable through one export documented as test-only.

Rust: the opencode snapshot parser became the shared `plugin_live.rs`. The Pi adapter reads snapshots, reports live state, routes only `session_start:*` and first-version payloads to attribution, and trusts the extension's pid for snapshots and attribution records. `hook status pi` names an older copy's version, says a pre-2 copy reports no live state (health `off`), and gives the floor: waiting needs Pi 0.84.4, 1.1.0 recommended. ADR 0206 decisions 25 to 28 and an ADR 0121 amendment record the choices. The extension README, operations guide, design, and CHANGELOG are updated.

Verified: `just pi-plugin-test` passes 31 tests on Node 24.21. They replay 18 synthetic scripts and run the default export against a stand-in Pi and writer, covering unref'd timers, a quit's report outliving Pi, and a missing binary. The TypeScript was executed under node and never compiled against Pi's types. The Rust tests replay every emitted payload through the parser and store, including heartbeats, quiet, kill, reload, switches, and out-of-order writers. Sentinel tests in `pi_live_sequence_tests.rs` and `tests/hook_write.rs` show no error text, tool name, or event text reaches a snapshot, the store, the spool, or an output. `hook status pi` is covered in `cli_smoke.rs`. `run-tests.sh` passed (5346 passed, 0 failed), as did `checks.sh` (fmt, clippy, and rustdoc with warnings denied, plus diff --check). nextest and `just tuikit-features` were not rerun because tuikit is untouched. Nine `tui --snapshot` frames are byte-identical to the baseline.

Follow-ups: CSP-850 (configurable subagent tools) and CSP-851 (send Pi's version; needs CSP-822). The Pi claims to confirm live were added to CSP-822.
<!-- SECTION:FINAL_SUMMARY:END -->
