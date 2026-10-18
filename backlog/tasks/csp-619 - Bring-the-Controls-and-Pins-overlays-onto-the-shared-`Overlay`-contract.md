---
id: CSP-619
title: Bring the Controls and Pins overlays onto the shared `Overlay` contract
status: Done
assignee:
  - '@claude'
created_date: '2026-10-17 03:09'
updated_date: '2026-10-18 00:25'
labels:
  - h-hyg
milestone: m-11
dependencies: []
references:
  - docs/code-hygiene-audit.md
priority: medium
type: chore
ordinal: 677000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
CSP-497 gave TUI overlays one `Overlay` trait on a modal stack, so a new modal needs no new loop, keymap, or draw branches. Controls and Pins never joined it: their key handlers read a live context (`ControlsContext`, `PinsContext`) borrowed from `App` at event time, and CSP-497 left that for a later story that was never filed. The cost shows in `src/tui/runtime`, where each of the two overlays has a live handler and a fixture-mode copy. `static_handle_controls_overlay_key` (`src/tui/runtime.rs:906`) repeats `handle_controls_overlay_key` (`src/tui/runtime/overlay_keys.rs:323`) and discards its extra arguments with `let _ = (config, snapshot);`. `static_handle_pins_overlay_key` (`overlay_keys.rs:401`) repeats `handle_pins_overlay_key` (`overlay_keys.rs:371`) except for where the committed message goes (`static_apply_pins_msg` instead of `dispatch_live`). CSP-498 shared the loop skeleton through `LoopMode` but left these mode-specific paths. Audit finding A7.

- Scope: let the `Overlay` contract receive per-event context (for example an associated context type, or a context argument the modal stack supplies), move Controls and Pins onto it, and send their committed messages through the active `LoopMode`'s dispatch so the `static_*` overlay copies and `static_apply_pins_msg` can go. Rename and Search keep their specialized handlers unless the same change covers them cheaply.
- Tests: existing controls, pins, and fixture-mode (ADR 0069) runtime tests pass; one test drives a pins commit through both loop modes and checks they reach the same reducer message.
- Manual checks: `cargo run --features snapshot -- tui --snapshot --snapshot-keys "f"` and `--snapshot-keys "p"` render the Controls and Pins overlays unchanged.
- Blockers: none.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [x] #1 Controls and Pins implement the shared `Overlay` contract and dispatch through the modal stack like the other overlays
- [x] #2 `src/tui/runtime` has no fixture-mode copy of an overlay key handler; live and fixture modes differ only in the dispatch they supply
- [x] #3 Controls and Pins behave the same in live and fixture modes, covered by tests
<!-- AC:END -->

## Definition of Done
<!-- DOD:BEGIN -->
- [x] #1 `just check` passes, or `git diff --check` for docs-only changes
- [x] #2 Docs and ADRs are updated when behavior, architecture, or workflow change
<!-- DOD:END -->

## Implementation Plan

<!-- SECTION:PLAN:BEGIN -->
Premise correction: Controls and Pins already implement `Overlay` with a context (`impl crate::tui::Overlay for ControlsOverlayState` / `PinsOverlayState`); the filing-time grep missed the full-path form. What remains is the fixture-mode handler copies.
1. Delete `static_handle_controls_overlay_key`; it is `handle_controls_overlay_key` plus an always-`Ok` `Result`. Fixture mode calls the shared handler, which also drops its unused `config` parameter.
2. Replace `handle_pins_overlay_key` and `static_handle_pins_overlay_key` with one `route_pins_overlay_key(app, key, deliver)`; live mode passes `dispatch_live`, fixture mode passes `static_apply_pins_msg` (which drops its always-`Ok` `Result`).
3. Tests: a pins commit routed through the shared function reaches the supplied sink with the same `Msg` under both sinks; existing controls/pins/fixture tests pass; snapshot of the Controls and Pins overlays unchanged.
<!-- SECTION:PLAN:END -->

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
- Premise correction: Controls and Pins already implemented `Overlay` with a per-event context (`impl crate::tui::Overlay for ControlsOverlayState` / `PinsOverlayState`), so criterion 1 held before this work. The filing-time grep only matched `impl Overlay for` and missed the full-path form.
- Deleted `static_handle_controls_overlay_key` (the live handler plus an always-`Ok` `Result` and `let _ = (config, snapshot);`). Both loop modes and snapshot mode call `handle_controls_overlay_key`, which also lost its unused `config` parameter.
- `route_pins_overlay_key(app, key, deliver)` replaces `handle_pins_overlay_key` and `static_handle_pins_overlay_key`. Live mode supplies `dispatch_live`; fixture mode supplies `static_apply_pins_msg`, which no longer returns an always-`Ok` `Result`.
- Not changed: `tui/snapshot.rs` keeps its own pins key handler on the widget's older `handle_key` API with a stricter "skip every apply" policy. It lives outside `src/tui/runtime`, and moving it would change what snapshot mode renders.
- Validation: new `pins_overlay_commits_reach_the_sink_each_loop_mode_supplies` drives the same keys through both sinks; `tui --snapshot --snapshot-fixture tests/fixtures/showcase.json` renders of `f`, `f<Down><Down><Enter>`, `p`, and `p<Down><Down><Enter>` are byte-identical to the previous commit; release-profile `cargo check` clean; `just check` green (2402 tests).
<!-- SECTION:NOTES:END -->

## Final Summary

<!-- SECTION:FINAL_SUMMARY:BEGIN -->
The fixture-mode copies of the Controls and Pins overlay key handlers are gone. Controls has one handler for every mode; Pins has one `route_pins_overlay_key` that takes the mode's dispatch as an argument, so live and fixture modes differ only in where a committed message goes. Controls and Pins already implemented the shared `Overlay` contract (the ticket's premise there was a grep miss). Verified with a new routing test, byte-identical overlay snapshots against the previous commit, and `just check` (2402 tests).
<!-- SECTION:FINAL_SUMMARY:END -->
