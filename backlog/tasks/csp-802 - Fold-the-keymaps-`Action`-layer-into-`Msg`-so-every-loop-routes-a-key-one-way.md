---
id: CSP-802
title: Fold the keymap's `Action` layer into `Msg` so every loop routes a key one way
status: To Do
assignee: []
created_date: '2026-10-31 07:30'
labels:
  - tui-framework
  - h-tui
dependencies: []
documentation:
  - docs/plans/tui-framework/runtime-evaluation.md
  - docs/adr/0085-tui-mvu-architecture.md
priority: low
type: enhancement
ordinal: 888000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Found by the layer 3 runtime evaluation (CSP-765, `docs/plans/tui-framework/runtime-evaluation.md`). ADR 0085 contract 1 asks for one `Msg` and one `update`, and contract 5 for one loop whose modes differ only in their subscriptions. A key still resolves to the keymap's `Action` (23 variants: `Msg`, `ModalKey`, `Refresh`, `HardRefresh`, `Attach`, `Resume`, `View`, `DefaultAction`, the openers, view and grouping cycling, `CopySessionId`), and three dispatchers turn it into handlers: `LiveMode::dispatch` (92 lines), `StaticMode::dispatch_action` (111), and the snapshot driver's `apply_action` (54). They have drifted: the fixture TUI refuses attach with its own status text, `--snapshot` skips `Attach`, `Resume`, and `DefaultAction` as unsupported, and `r` reloads a fixture in one mode and runs real discovery in another. CSP-716 already fixed one round of fixture-mode drift of this kind.

Folding the variants into `Msg` (and `r` and `Ctrl-R` into refresh effects) lets each mode differ only in its executor, the way the pure executor already refuses a modal pick it cannot run. It is also the precondition for any later extraction of the runtime into the framework (ADR 0200). Related: CSP-391 (a command registry for the palette) would enumerate the same command surface.

Visible changes to catalog (ADR 0151): the fixture and scenario TUI's refusals ("scenario TUI is static; attach is disabled") and the snapshot driver's skip notes become the pure executor's wording.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 Every key the main screen takes resolves to messages; no `Action` variant other than a modal key remains, or the type is gone
- [ ] #2 The live loop, the fixture loop, and `--snapshot` hand a key to the same function, and differ only in the executor that runs its effects
- [ ] #3 `r` and `Ctrl-R` emit refresh effects that each mode's executor carries out (a worker live, a fixture reload in fixture mode, a rebuild or a note in a snapshot)
- [ ] #4 The `translate_*` tests and the tests that call `press_in_fixture_mode` assert messages; a test shows a fixture-mode attach refused with the pure executor's wording
- [ ] #5 The final summary lists every changed refusal text as a regression or a consistency win
<!-- AC:END -->

## Definition of Done
<!-- DOD:BEGIN -->
- [ ] #1 `just check` passes, or `git diff --check` for docs-only changes
- [ ] #2 Docs and ADRs are updated when behavior, architecture, or workflow change
<!-- DOD:END -->
