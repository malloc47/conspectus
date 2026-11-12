---
id: CSP-807
title: Add the neutral live-state vocabulary and rules to the model
status: To Do
assignee: []
created_date: '2026-11-12 01:47'
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
- [ ] #1 `LiveState`, `LiveDetail`, `LiveObservation`, `EffectiveLive`, and `LiveCounts` exist in `src/model/live.rs` with serde and rkyv support
- [ ] #2 `effective()` implements ADR 0202 decision 3, with a test at each boundary
- [ ] #3 An unknown state name read from disk deserializes to `unknown` instead of failing
- [ ] #4 The module names no harness, and `docs/library-api.md` lists the new public types
<!-- AC:END -->

## Definition of Done
<!-- DOD:BEGIN -->
- [ ] #1 `just check` passes, or `git diff --check` for docs-only changes
- [ ] #2 Docs and ADRs are updated when behavior, architecture, or workflow change
<!-- DOD:END -->
