---
id: CSP-765
title: 'Optional stretch: evaluate extracting the application runtime (layer 3)'
status: Done
assignee:
  - '@claude'
created_date: '2026-10-27 20:28'
updated_date: '2026-10-31 07:36'
labels:
  - tui-framework
  - tuifw-close
milestone: m-25
dependencies:
  - CSP-729
  - CSP-731
  - CSP-732
  - CSP-757
documentation:
  - docs/tui-architecture-review.md
  - docs/plans/tui-framework/inventory/06-interaction-conventions.md
priority: low
type: spike
ordinal: 851000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
ADR 0150 leaves the application runtime in Conspectus: the MVU event loop, the effect lifecycle, undo, and terminal suspend and resume for subprocess hand-off. Once layers 1 and 2 are in the framework, it is worth knowing how much of the runtime is generic and whether extracting it would pay for itself. `docs/tui-architecture-review.md` records what changed since the review (effects as data, the modal stack, the declarative keys), and the interaction-conventions inventory marks the effect lifecycle and undo classifier as "layer 3 or domain".

Scope: Measure and write up: which runtime code is generic (the poll and render loop, toast ticking, the exhaustive effect-classifier pattern, terminal suspend and resume) and which knows the graph model (effects, undo, the reducer's data); what a `Runtime<M, Fx>` seam would look like; the cost in stories; and the benefit for a second consumer. Recommend go, no-go, or later, and if go, outline the stories.

TUI changes: none; this is an evaluation.

Tests and checks: The write-up lists the files and line counts that fall on each side. It is reviewed against the framework's layer 2 contracts.

Blockers: Optional: only worth doing if the extraction work has gone smoothly and the operator wants a second consumer.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [x] #1 A write-up classifies the runtime code as generic or domain, with files and line counts
- [x] #2 It sketches the runtime seam and estimates the stories
- [x] #3 It recommends go, no-go, or later, with reasons
<!-- AC:END -->

## Definition of Done
<!-- DOD:BEGIN -->
- [x] #1 `just check` passes, or `git diff --check` for docs-only changes
- [x] #2 Docs and ADRs are updated when behavior, architecture, or workflow change
<!-- DOD:END -->

## Implementation Plan

<!-- SECTION:PLAN:BEGIN -->
1. Read the task, ADRs 0024/0085/0150, the architecture review (F1-F7), inventory slice 06 sections 4 and 7, and the framework's guide and architecture pages for what layer 2 already gives a host.
2. Measure the current runtime: every file under src/tui/runtime*, effect.rs, undo.rs, snapshot.rs, app.rs and app/*, plus the framework testkit pieces, with wc -l and a per-item line count of runtime.rs (measurement script in /tmp/tuifw-baseline, not committed).
3. Classify each file and each region of the mixed files as generic or domain; table with line counts and percentages.
4. Sketch a Runtime<M, Fx> seam in Rust signatures; check it against the two loops (live, fixture), snapshot mode, and each layer-2 contract (Overlay/ModalStack, keys, feedback, AppFrame/Screen, testkit).
5. Estimate the extraction in stories (sizes, dependencies) and list the risks; weigh the benefit for a second consumer and whether one exists.
6. Recommend go, no-go, or later with the trigger; record the decision in an ADR if it is significant; record judgment calls in operator-review.md; link the evaluation from the plan README.
7. git diff --check (docs only); finalize the task.
<!-- SECTION:PLAN:END -->

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
Measured at 1aca4148 with wc -l and a throwaway line classifier in /tmp/tuifw-baseline/765 (not committed): every line of the runtime's files is generic, domain, or wiring; mixed items split by line. Runtime proper 5,899 lines: 643 generic, 4,968 domain, 288 wiring (11.5% generic); with the reducer and App 6.2%; tests 7.0%. runtime.rs's 1,900 domain lines are grouped by concern with no line left over.
Seam sketched as Program / Executor / Exec / TerminalGuard / EventSource / Runtime<M, Fx> / Headless, checked against the live loop, the fixture loop, snapshot mode, and each layer 2 contract (overlay and ModalStack, keys, feedback, AppFrame and Screen, testkit, paste rule).
Recommendation LATER with a trigger, recorded as ADR 0200 and restated in the framework's record 0002. The Action-layer fold (three dispatchers, 257 lines) filed as CSP-802 under the brief's out-of-scope rule.
Checks: git diff --check clean; cargo test -p tuikit --test doc_sync --test independence pass (24 and 37 tests, host-agreement check ran).
<!-- SECTION:NOTES:END -->

## Final Summary

<!-- SECTION:FINAL_SUMMARY:BEGIN -->
Wrote `docs/plans/tui-framework/runtime-evaluation.md`, an evaluation of extracting the application runtime (layer 3). Recommendation: **later**.

- **Classification** (AC 1): a table of 22 files with measured lines on each side. The runtime proper (`runtime.rs`, `runtime/`, `effect.rs`, `undo.rs`, `snapshot.rs`, `keymap.rs`) is 5,899 lines: 643 generic, 4,968 domain, 288 wiring, so 11.5% generic; 6.2% counting the reducer and `App`. The generic code is broken down by concern (the loop, input and drawing, the terminal guard and hand-off frames, the spinner, the effects-as-data skeleton, modal routing, the bounded undo stack, the headless driver), and the code that is generic in shape but domain in content is listed apart (the lifecycle and undo classifiers, the worker ledger, three copies of the `Action` dispatch, 257 lines).
- **Seam and stories** (AC 2): `Program`, `Executor`, `Exec`, `TerminalGuard`, `EventSource`, `Runtime<M, Fx>`, and `Headless` in Rust signatures, checked against the live loop, the fixture loop, `--snapshot`, and each layer 2 contract. The contracts need nothing new; the precondition is host-side (fold `Action` into `Msg`). Nine stories (three L, five M, one S) with dependencies, and the risks: worker threads and the worker ledger, 27 signatures taking `&mut DefaultTerminal`, three hand-off rituals covered only by the 32 PTY tests, 48 test call sites on runtime helpers, and the drift in fixture and snapshot refusal texts.
- **Recommendation** (AC 3): later. There is no second consumer, a simple one needs about 30 lines of loop, and ratatui already restores the terminal on a panic. The trigger: a second program committed to run on `tuikit` with a loop of its own that needs a terminal hand-off, background work, or a headless mode. When it fires, the terminal guard comes first, as an opt-in layer (a sibling crate is preferred). Recorded as ADR 0200, Accepted; ADR 0150's status and the index point at it; `docs/design.md` and the framework's record 0002 restate it.
- Filed CSP-802 (To Do, Low, no milestone): fold the keymap's `Action` layer into `Msg` so the three loops route a key one way. It pays without any extraction. No outline tasks were filed, since the recommendation is not go.
- The plan README links the evaluation and lists CSP-802. `operator-review.md` has four CSP-765 rows: the recommendation, the counting rules, the seam's defaults, and filing CSP-802.

**Regressions:** none; no TUI change.
**Consistency wins:** none; no TUI change.

Verified: `git diff --check` is clean; `cargo test -p tuikit --test doc_sync --test independence` passes (24 and 37 tests; the host-agreement check ran). The line counts come from `wc -l` and a throwaway classifier kept under `/tmp/tuifw-baseline/765`, not committed.
<!-- SECTION:FINAL_SUMMARY:END -->
