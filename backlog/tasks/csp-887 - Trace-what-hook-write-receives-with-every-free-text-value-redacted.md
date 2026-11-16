---
id: CSP-887
title: 'Trace what hook write receives, with every free-text value redacted'
status: Done
assignee:
  - '@claude'
created_date: '2026-11-16 17:51'
updated_date: '2026-11-16 18:38'
labels:
  - live-state
  - ls-emit
milestone: m-26
dependencies: []
references:
  - docs/adr/0086-payload-privacy-tenet.md
  - docs/adr/0204-the-live-state-store-folds-agent-events-per-session.md
  - >-
    docs/adr/0242-a-hook-trace-records-what-hook-write-receives-with-every-free-text-value-redacted.md
priority: medium
type: enhancement
ordinal: 964000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
CSP-822 needs the payloads the installed hooks really receive from Claude Code, Codex, Pi, and OpenCode, to compare real event sequences with reducers that were built from documentation. A raw payload carries prompts, tool input and output, paths, and session ids, and ADR 0086 keeps those off disk, so a recording has to be redacted before it is written, in the one place all four harnesses already reach: `conspectus hook write`. Setting `CONSPECTUS_HOOK_TRACE=<name>` makes the writer append one redacted JSON line per call to a trace file in the state directory, without touching any harness configuration and without changing what the hook does. The line also records the neutral events the adapter produced, what the store did with them, and the state the reducer computed, so a later reader can tie each real event to its result.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [x] #1 ADR 0242 records the decision (why a trace in the writer, default-deny redaction, the state directory, the ADR 0086 tier, operator-initiated only, the alternatives), is indexed in `docs/adr/README.md`, and `docs/capabilities.toml` cites it
- [x] #2 With `CONSPECTUS_HOOK_TRACE` set to a name matching `^[A-Za-z0-9][A-Za-z0-9._-]{0,63}$`, every `conspectus hook write <harness>` appends one JSON line to `$XDG_STATE_HOME/conspectus/hook-trace/<name>.jsonl` (directory 0700, file 0600, one `O_APPEND` write); unset or invalid, no file is touched
- [x] #3 Each line has `v`, `t_ms`, `harness`, `payload` (redacted), `env` (booleans for `TMUX_PANE`, `TMUX`, `CLAUDE_PID`), `events` (neutral kind names, or `snapshot`), `applied`, and `after` (the stored observation read back after the apply)
- [x] #4 Redaction is default-deny and keeps structure: numbers, booleans, nulls, well-formed keys to depth 3, array lengths (8 elements at most), enum values on a whitelist of keys, non-MCP tool names, salted id pseudonyms, and size-only placeholders for every other string, path, and known payload-bearing value
- [x] #5 Tracing never changes the hook: stdout and stderr stay empty, the exit code stays 0, the store writes are the same, every trace error is swallowed, and no process or socket is added; the median time with and without the trace is measured
- [x] #6 Tests cover each redaction rule, pseudonym stability within a trace and difference across traces, file and directory modes, an invalid name, an unwritable trace path, `after` against the store for a Claude Code sequence, and a sentinel run of every hook fixture of all four harnesses through the full command with the trace on
- [x] #7 `docs/operations.md` has a section "Recording what a harness sends" (setting the variable, where the file goes, a line, what is kept, deleting traces)
<!-- AC:END -->

## Definition of Done
<!-- DOD:BEGIN -->
- [x] #1 `just check` passes, or `git diff --check` for docs-only changes
- [x] #2 Docs and ADRs are updated when behavior, architecture, or workflow change
<!-- DOD:END -->

## Implementation Plan

<!-- SECTION:PLAN:BEGIN -->
1. ADR 0242: the trace inside `hook write`, default-deny redaction, the state directory, the ADR 0086 tier, operator-initiated only, the direct `sha2` dependency (already in the lock), the alternatives. Index it, note it in ADR 0206, cite it in `docs/capabilities.toml`.
2. `src/hook/redact.rs`: pure redaction of a payload `serde_json::Value` (key shape and depth, array and key caps, enum whitelist, tool names, id pseudonyms, paths, payload-bearing keys), with `redact_tests.rs`.
3. `src/hook/trace.rs`: the variable, the name check, the trace line, and the append (`O_APPEND`, modes 0700/0600, errors swallowed); `LiveEventKind::label` and `Applied::word` supply the words.
4. Wire it into `HookWriteArgs::write` after the store write and attribution: `write_payload` returns what it applied; tracing off costs one environment lookup.
5. Tests: unit tests per rule; `tests/hook_trace.rs` runs every fixture of all four harnesses through the command with the trace on and scans the file for sentinels, raw ids, paths, and text; modes, invalid name, unwritable trace path, `after` against the store, tracing off.
6. Docs: `docs/operations.md` section, CHANGELOG, capabilities manifest. Measure the median `hook write` time with and without the trace.
<!-- SECTION:PLAN:END -->

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
Verification: `just check` exit 0 (fmt, clippy, nextest, doctests, rustdoc, the framework matrix); the suite with no `conspectus` on PATH, 5874 passed, 1 skipped. `tests/hook_trace.rs` runs 1717 fixture payloads of all four harnesses (as-is and with sentinels planted in every ID, path, text, and odd key) through the real command with the trace on; `tests/capabilities.rs` traces a traced state event under strace (writes inside `$STATE/conspectus/**`, no process, no socket). Cost on a release build, median of 200 interleaved runs: state event 4.44 ms off, 4.45 ms on; session start 4.27 and 4.61 ms; 1 MB tool response 6.57 and 7.96 ms.
Found while testing: `error` is a failure's type only in a `StopFailure`, so it is vocabulary only there; a word-shaped value under any other `error` is text and is redacted.
<!-- SECTION:NOTES:END -->

## Final Summary

<!-- SECTION:FINAL_SUMMARY:BEGIN -->
Landed `CONSPECTUS_HOOK_TRACE=<name>` (ADR 0242): `conspectus hook write` appends one redacted JSON line per call to `$XDG_STATE_HOME/conspectus/hook-trace/<name>.jsonl` (directory 0700, file 0600, one `O_APPEND` write), after the store write and the attribution record. A line holds `v`, `t_ms`, `harness`, the redacted `payload`, `env` booleans, the neutral `events` labels (or `snapshot`), `applied`, and `after`, the stored observation read back right after the apply (state, detail, subagents, via_subagent, since_ms, rev). Redaction (`hook::redact`) is default-deny: unknown strings are `<str:N>`, ids are salted SHA-256 pseudonyms (`sha2`, already in the lock, now a direct dependency), paths `<path>`, MCP tool names `mcp__<redacted>`, payload-bearing values sizes, and `error` is vocabulary only in a `StopFailure`. Unset or invalid names cost one environment lookup and touch no file; every trace error is swallowed.

Verified: `just check` exit 0; the suite with no `conspectus` on PATH, 5874 passed. Unit tests per rule (`redact_tests.rs`, `trace_tests.rs`, `event_tests.rs`, `hook_write_tests.rs`); `tests/hook_trace.rs` runs 1717 fixture payloads of all four harnesses, as-is and with sentinels planted in every id, path, text, and odd key, through the real binary and finds no sentinel, raw id, path, or text in any trace; modes, invalid names, an unwritable trace path, `after` against the store for a Claude Code sequence, and a trace-off run; `tests/capabilities.rs` runs a traced state event under strace. Medians on a release build (200 interleaved runs): state event 4.44 ms off and 4.45 ms on, session start 4.27 and 4.61 ms, 1 MB tool response 6.57 and 7.96 ms. CSP-822 can now consume traces.
<!-- SECTION:FINAL_SUMMARY:END -->
