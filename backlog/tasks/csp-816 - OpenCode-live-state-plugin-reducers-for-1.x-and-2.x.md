---
id: CSP-816
title: 'OpenCode live state: plugin reducers for 1.x and 2.x'
status: Done
assignee:
  - '@claude'
created_date: '2026-11-12 01:47'
updated_date: '2026-11-12 11:20'
labels:
  - live-state
  - ls-emit
milestone: m-26
dependencies:
  - CSP-810
documentation:
  - docs/plans/live-state/research/03-opencode.md
  - >-
    docs/adr/0206-live-state-emitters-subscribe-to-a-minimal-event-set-per-harness.md
priority: medium
type: enhancement
ordinal: 902000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
The OpenCode plugin forwards the event name only, which cannot tell busy from idle, and keeping state needs a reducer inside the plugin because OpenCode publishes permission asks and questions as events that no storage poll can see. 2.x also needs a server entrypoint so one writer sees every session, and the research found two plugin defects.

Scope: Extend the 1.x plugin with a per-session reducer that sends a complete snapshot (`phase`, pending permission/question/form counts, outcome, retry, parent) on change plus a 60 s heartbeat while not idle; add the 2.x server entrypoint (amending ADR 0126) listening to the events of ADR 0206; raise a child session's wait to its root; fix the 1.x forwarding of child-session events to the attribution path and the dead `session.compacted` filter entry in 2.x. Rust side: the OpenCode `LiveEvent` snapshot mapping with the whitelist and fixtures. Since the dev shell has no Node, verify the reducer through the event-script fixtures replayed by a Rust mirror in tests, and record the manual checks.

TUI changes: none visible. The TUI renders the same frames before and after (check a few views with `conspectus tui --snapshot`).

Tests and checks: Payload fixtures (busy, retry, permission, question, error, child, legacy), event-script fixtures with expected snapshots, a privacy test that no `resources`, `patterns`, or message survives, child-to-root rollup, `seq` ordering within one second, and the plugin build still reproducible (committed output).

Blockers: The fast writer.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [x] #1 The plugin sends a state snapshot on change and a heartbeat while not idle, with `instance` and `seq`
- [x] #2 A child session's wait raises `waiting` with `via_subagent` on its root session
- [x] #3 The two plugin defects from the research are fixed
- [x] #4 Fixtures and a Rust mirror verify the reducer without Node, and the manual checks are recorded
<!-- AC:END -->

## Definition of Done
<!-- DOD:BEGIN -->
- [x] #1 `just check` passes, or `git diff --check` for docs-only changes
- [x] #2 Docs and ADRs are updated when behavior, architecture, or workflow change
<!-- DOD:END -->

## Implementation Plan

<!-- SECTION:PLAN:BEGIN -->
1. Reducer (plugins/opencode-hook/src/live.ts): pure module, erasable TypeScript only, no imports. Per-session state (base state with since, pending permission/question/form ids with start times, a pending 1.x failure or interrupt, the state compaction interrupted, parent id, cwd). 1.x and 2.x event parsers onto one set of transitions; derived snapshot by research 7.2 precedence mapped onto ADR 0203 (own waits, else a descendant's raised with via_subagent; compacting; working/retrying|rate_limited|generating; error/<class>; ready/finished|interrupted; exited/ended). Emits whole payloads ({session_id, cwd, hook_event_name, attribution?, live:{instance, seq, observed_ms, heartbeat_s, pid, pid_start?, state, detail?, subagents, via_subagent, since_ms}}) only for sessions whose derived snapshot changed, plus heartbeat() and dispose(). The 1.x attribution rule lives here too (root sessions' lifecycle events only).
2. Shared JSONL event scripts under tests/fixtures/hooks/opencode-live/ (synthetic, README): busy->idle, retry then rate limit, permission ask/reply/reject, question, error, child waiting raises root, foreground and background subagent, compaction, interrupt, seq within one second, heartbeat, dispose, 2.x location filter; sentinels in every payload-bearing field. node --test tests in plugins/opencode-hook/test/ replay every script and compare payloads; `npm test` script and a `just plugin-tests` recipe that says when node is missing.
3. Glue: 1.x index.ts feeds events, the compaction hook, and dispose to the reducer; 2.x new v2/server.ts subscribes with ctx.event.subscribe in the service; write.ts sends detached unref'd spawns and runs the heartbeat timer; tui.ts drops the dead session.compacted entry. Not compiled here (no npm install).
4. Rust: OpenCode adapter module opencode_live.rs: is_attribution_event (explicit `attribution` flag, else legacy payloads without `live`), live_snapshot_from_payload whitelist parse (unknown state -> unknown, unknown detail dropped, counts clamped, times clamped, pid from the plugin), reports_live_state true. Tests: every expected payload through the parser and the store with effective states; seq and instance ordering; privacy sentinels; legacy payloads attribution-only; integration test through hook write.
5. Docs: ADR 0126 amendment (server entrypoint for state), ADR 0049 amendment text, ADR 0206 as-built section, plugin README, package.json, operations.md OpenCode live state, CHANGELOG, plan README bug list. Check tui --snapshot frames byte-identical, checks.sh, run-tests.sh; commit by concern; finalize.
<!-- SECTION:PLAN:END -->

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
Planning note (after CSP-809): plugin sources do not use the shared reducer. They send a `LiveSnapshot` (`src/live_store/`), which the store applies by `(instance, seq)`. Read `LiveStore::apply_snapshot` and the `LiveSnapshot` type first.

Planning note (after CSP-810): the writer seam is on `HarnessAdapter` (`src/discovery/harness/mod.rs`): `live_events_from_payload(payload, context, observed_ms)` for event sources, `live_snapshot_from_payload` for plugin snapshots, `live_reducer` for a harness reducer, and `is_attribution_event`, which defaults to `true`. Override `is_attribution_event` so only the events that name the session (session start) take the spool and daemon path; every other event then reaches only the live-state store. `hook write` (`src/cli/hook_write.rs`) fills pid, pid start, and tmux pane/socket/session into the context; the adapter adds cwd and transcript path from its payload view. Claude Code's `LiveHook` view in `claude_code.rs` is the pattern (borrowed serde struct of whitelisted fields).

Verification approach (replaces the Rust mirror in the scope): write the reducer as a pure TypeScript module (for example `src/live.ts`) with no imports from the harness package and no side effects on import, and test it with `node --test` (Node 24 strips types; the machine has `node` in the user profile, the dev shell does not). Shared event-script fixtures (JSONL: one OpenCode event per line, with the snapshot the reducer must emit after it) are consumed by the TypeScript tests and by the Rust tests that feed the expected snapshots through the payload parser and the store. Add a `just` recipe for the plugin tests that reports "node not found" cleanly. ADR 0206 describes this.

Built in four commits: the pure reducer with its node tests and the shared event scripts, the plugin glue, the Rust parser and its tests, and the docs.

Verification (the approach of the planning note above, which replaced the Rust mirror):
- `node --experimental-strip-types --test test/live.test.ts` in plugins/opencode-hook (Node 24.21): 27 tests pass. 19 scripts (12 for 1.x, 7 for 2.x) replay through src/live.ts; every emitted payload must equal the script's `emit`, seq counts up by one, no payload holds a SENTINEL, and each step's hand-written `shown` matches the latest snapshot per session. `just opencode-plugin-test` runs the same and exits 1 with "node not found on PATH" when node is missing (checked with a PATH without node).
- Rust: opencode_live_tests.rs (parser whitelist, clamps, attribution rule, process precedence, sentinels through the parser and the store) and opencode_live_sequence_tests.rs (every emitted payload of every script through the adapter and the store with effective states checked after every step, seq order within one second, a reloaded instance, legacy payloads). tests/hook_write.rs: no SENTINEL in any file or output after every emitted payload, a planted variant, and every raw event; a snapshot touches no socket, no tmux, and no spool; a legacy payload is attributed as before.
- Manual checks, run here: every plugin file passes `node --experimental-strip-types --check` (erasable syntax only). A scratch copy of the plugin (imports rewritten to .ts) ran under Node 24 against the worktree's real `conspectus` with a scratch HOME and XDG dirs: the 1.x plugin function fed events, the compaction hook, and dispose; the 2.x server setup() fed a stand-in ctx.event.subscribe stream, then its cleanup. The store showed the expected states (child waiting raised on the root with via_subagent, compacting, exited/ended after dispose), the pid and /proc start time of the node process, no SENTINEL anywhere, the spool holding only root sessions' attribution records and nothing from the 2.x server; node exited at once with sessions still working (timer and writers unref'd); with the interval patched to 1 s, three heartbeats were sent while working and none once ready.
- Not done: the TypeScript glue was never compiled (no npm install here): run `npm install && npm run typecheck` in plugins/opencode-hook. Neither real opencode nor Bun ran it (CSP-822).
- `tui --snapshot` frames of nine views are byte-identical before and after. checks.sh passes; run-tests.sh: 5304 passed, the two known `discovery::forge` gh-spawn flakes failed once and passed on rerun.

Decisions for review:
- Attribution rule: a payload takes the attribution path when it carries `attribution: true`, or carries no `live`. The 1.x plugin marks a root session's created/updated/status/idle/compacted (the events it forwarded before), so attribution cost and in-app-switch behavior are unchanged and child sessions are never attributed; the 2.x server marks nothing; the 2.x TUI is unchanged. A new plugin with an older binary attributes every snapshot, so docs say to update both together.
- 2.x live state comes from a new server entrypoint (dist/v2/server.js), amending ADR 0126 decision 3; it filters events to its own location (trailing slashes ignored) and listens to session.step.started and session.status beyond ADR 0206's list. A cli.json-only install gives attribution without live state; the TUI-store fallback is CSP-846.
- Compaction ends in the state it interrupted (as the shared reducer), not always busy as the research table says; a 1.x status event ends compaction.
- A form that is not the question tool reads waiting/input (no form detail in the vocabulary); a `global` form is ignored.
- interrupted{shutdown} reads exited/ended; superseded and inactivity read ready/interrupted.
- Dispose (1.x dispose, 2.x unload) ends sessions still working, compacting, or waiting; on a reload without restart they read exited until their next event.
- Heartbeats go to working, waiting, compacting, and error sessions (ADR 0206: not ready).
- Error classes: 1.x by error name (MessageAbortedError interrupted, ProviderAuthError auth) and API status (429, 401/403, 503/529); 2.x by status, retryAfterMs, then type words. Rate-limited retries: 1.x action reason, 2.x status 429 or retryAfterMs.
- Child sessions get entries of their own beside the rollup on their root. A parent with no evidence of its own sends unknown when a raised wait ends.
- The plugin's pid and /proc/self/stat start time replace the writer's parent walk; the writer's start time wins when both name the same pid.
- Writes are spawned detached (so dispose writes land as opencode exits); see the note added to CSP-845.
- The sessions an instance remembers are capped at 256, dropping the least recently changed quiet ones.
- The plugin version is 0.3.0.
<!-- SECTION:NOTES:END -->

## Final Summary

<!-- SECTION:FINAL_SUMMARY:BEGIN -->
Landed OpenCode live state for opencode 1.x and 2.x. The plugin (0.3.0) keeps a per-session reducer in a pure module, `plugins/opencode-hook/src/live.ts`, and sends each session's whole snapshot (`instance`, `seq`, state, detail, subagents, `via_subagent`, since, pid and start time) through `conspectus hook write opencode` on every change, plus a 60 s heartbeat while a session is not ready. A child session's wait raises its root with `via_subagent`. The 1.x entrypoint feeds it events, the compaction hook, and `dispose`. A new 2.x server entrypoint (`dist/v2/server.js`, amending ADR 0126 decision 3) feeds it the service's events for its location. Both research defects are fixed: only root sessions' 1.x lifecycle events take the attribution path, and the 2.x TUI no longer lists `session.compacted`. On the Rust side the opencode adapter whitelist-parses the `live` object (`opencode_live.rs`). A payload is attributed only when it is marked `attribution: true` or carries no live state, so 0.2.0 payloads work as before. `reports_live_state` is true.

Verified with Node 24: `node --test` replays 19 synthetic event scripts (`tests/fixtures/hooks/opencode-live/`, 27 tests). `just opencode-plugin-test` wraps the run and fails with a message when node is missing. Rust tests replay every emitted payload through the parser and the store with effective states, seq and instance ordering, and sentinel checks (unit and `hook write` end to end). The glue was also executed under Node against the real binary in a scratch home. It was not compiled (no npm install): run `npm run typecheck` in plugins/opencode-hook. `tui --snapshot` frames are byte-identical. checks.sh and run-tests.sh pass (two known gh-spawn flakes passed on rerun). The Rust mirror named in AC 4 was replaced by this approach, as the planning note records.

Follow-ups: CSP-846 (2.x TUI-only fallback), CSP-847 (re-read pending asks at server start), CSP-848 (hook status for opencode), CSP-849 (plugin CI, needs a decision).
<!-- SECTION:FINAL_SUMMARY:END -->
