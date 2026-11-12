---
id: CSP-807
title: Add the neutral live-state vocabulary and rules to the model
status: Done
assignee:
  - '@claude'
created_date: '2026-11-12 01:47'
updated_date: '2026-11-12 02:22'
labels:
  - live-state
  - ls-model
milestone: m-26
dependencies:
  - CSP-806
documentation:
  - docs/adr/0202-live-state-is-a-neutral-observation-with-a-closed-state-set.md
  - docs/plans/live-state/research/06-internal-audit.md
priority: high
type: enhancement
ordinal: 893000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Every surface (CLI, JSON, TUI, library) needs one meaning for "waiting", "ready", and "working", and one staleness rule, or the same session reads differently in different views. The vocabulary belongs in the provider-neutral model (ADR 0202) before any emitter or viewer is built, so the emitters map into it and the viewers read from it.

Scope: Add `src/model/live.rs` with `LiveState`, `LiveDetail` (and the valid state/detail pairs), `LiveObservation`, `Liveness` (`Alive`, `Dead`, `Unknown`), `LiveConfidence` (`fresh`, `quiet`, `stale`), `EffectiveLive`, `LiveCounts`, and a default precedence function. Implement `LiveObservation::effective(now_ms, liveness)` exactly as ADR 0202 decision 3 specifies (dead pid or changed start time, quiet window of 10 minutes or three heartbeats, no decay for waiting/ready/error while alive, the 15-minute rule with unknown liveness, starting after two minutes). Serde and rkyv derives, stable lowercase serialization names. No harness vocabulary in the module.

TUI changes: none visible. The TUI renders the same frames before and after (check a few views with `conspectus tui --snapshot`).

Tests and checks: Unit tests for every rule and each boundary (just inside and outside each window), heartbeat and no-heartbeat sources, `exited` never being revived by time, counts and precedence, serde round-trips and rejection of unknown states by a lenient reader that yields `unknown`. The model's independence tests still pass.

Blockers: The ADRs.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [x] #1 `LiveState`, `LiveDetail`, `LiveObservation`, `EffectiveLive`, and `LiveCounts` exist in `src/model/live.rs` with serde and rkyv support
- [x] #2 `effective()` implements ADR 0202 decision 3, with a test at each boundary
- [x] #3 An unknown state name read from disk deserializes to `unknown` instead of failing
- [x] #4 The module names no harness, and `docs/library-api.md` lists the new public types
<!-- AC:END -->

## Definition of Done
<!-- DOD:BEGIN -->
- [x] #1 `just check` passes, or `git diff --check` for docs-only changes
- [x] #2 Docs and ADRs are updated when behavior, architecture, or workflow change
<!-- DOD:END -->

## Implementation Plan

<!-- SECTION:PLAN:BEGIN -->
1. Add `src/model/live.rs` as the public module `model::live`: `LiveState` and `LiveDetail` (closed sets, stable snake_case names, the state/detail pairing table), `LiveSource`, `LiveObservation` (state, detail, subagents, via_subagent, since_ms, observed_ms, source, pid, pid_start, heartbeat_s), `Liveness` with a pid-reuse helper, `LiveConfidence`, `EffectiveLive`, `LiveCounts`, and the default precedence (waiting, error, ready, working, then compacting, starting, unknown, exited). Serde and rkyv derives as in `node.rs`; a lenient reader turns an unknown state or source name into `unknown` and an unknown detail into none.
2. Implement `LiveObservation::effective(now_ms, liveness)` per ADR 0202 decision 3, measuring every window from `observed_ms` (latest evidence), with "older than" meaning strictly past the window.
3. Unit tests in `src/model/live_tests.rs`: every rule with both sides of each boundary, heartbeat and event-only sources, exited never revived, pid reuse, pairing table, counts and precedence, serde and rkyv round trips, lenient reading, stable names, and a scan that the module names no registered harness.
4. Docs: list the types in `docs/library-api.md`, note the window reference in ADR 0202, CHANGELOG entry for the library surface.
5. Run narrow tests, `checks.sh`, and the full `run-tests.sh`; confirm TUI snapshot frames are unchanged; finalize the task.
<!-- SECTION:PLAN:END -->

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
Implemented `src/model/live.rs` (public module `model::live`) with unit tests in `src/model/live_tests.rs` (35 tests).

Decisions:
- Every window in `effective()` is measured from `observed_ms` (the latest evidence), not `since_ms`, so heartbeats and repeated events keep a state fresh; "older than" means strictly past the window. ADR 0202 decision 3 now says so.
- A recorded `exited` is returned as is under any liveness, so `exited/ended` never becomes `exited/killed` and time never revives it.
- With liveness `Unknown`, the quiet flag still applies before the 15-minute cutoff.
- The order after working in the default precedence is compacting, starting, unknown, exited.
- `LiveSource` has a lenient `unknown` fallback like `LiveState`; an unknown detail reads as none; a detail that does not fit its state is dropped when read.
- The changed-start-time rule lives in `Liveness::of_running_pid(recorded, current)`, since `Liveness` is a plain three-way answer.

Validation: `run-some.sh --lib model::live` 35 passed; `checks.sh` (fmt, clippy -D warnings, rustdoc -D warnings, git diff --check) exit 0; full `run-tests.sh` 4923 passed, 1 failed (`cli_history` rebuild_brings_a_killed_mux_back_with_its_recorded_command, a tmux "server couldn't be checked" flake) which passed on rerun (9/9), so 4924 = baseline 4889 + 35 new. `tui --snapshot` frames for sessions, session detail, mux, PRs, pinned, and history on the showcase fixture are byte-identical between the baseline binary and this one (scratch HOME, tmux and forge off). tuikit is untouched, so `just tuikit-features` was not rerun.
<!-- SECTION:NOTES:END -->

## Final Summary

<!-- SECTION:FINAL_SUMMARY:BEGIN -->
Landed `model::live` (`src/model/live.rs`), the vocabulary every live-state surface will share: the closed `LiveState`/`LiveDetail` sets with their valid pairs, `LiveSource`, `LiveObservation`, `Liveness` (with `of_running_pid` for pid reuse), `LiveConfidence`, `EffectiveLive`, `LiveCounts`, and `LiveState::default_precedence`. All of them derive serde and rkyv. `LiveObservation::effective(now_ms, liveness)` implements ADR 0202 decision 3, with every window measured from the latest evidence (ADR 0202 now says so). Unknown state and source names read `unknown`, and an unknown detail reads as none. The module names no harness. `docs/library-api.md` lists the types and CHANGELOG records the library surface. Nothing produces or shows live state yet.

Verified by 35 unit tests in `src/model/live_tests.rs` (each rule on both sides of each window, heartbeat and event-only sources, exited never revived, pairs, counts, precedence, JSON and rkyv round trips, lenient reading, and a scan for harness names), by `checks.sh` (fmt, clippy and rustdoc with warnings denied, git diff --check), and by the full suite: 4924 passed (baseline 4889 + 35), after one tmux flake in `cli_history` passed on rerun. `tui --snapshot` frames of six views on the showcase fixture are byte-identical before and after.
<!-- SECTION:FINAL_SUMMARY:END -->
