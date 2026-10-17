---
id: CSP-619
title: Bring the Controls and Pins overlays onto the shared `Overlay` contract
status: To Do
assignee: []
created_date: '2026-10-17 03:09'
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
- [ ] #1 Controls and Pins implement the shared `Overlay` contract and dispatch through the modal stack like the other overlays
- [ ] #2 `src/tui/runtime` has no fixture-mode copy of an overlay key handler; live and fixture modes differ only in the dispatch they supply
- [ ] #3 Controls and Pins behave the same in live and fixture modes, covered by tests
<!-- AC:END -->

## Definition of Done
<!-- DOD:BEGIN -->
- [ ] #1 `just check` passes, or `git diff --check` for docs-only changes
- [ ] #2 Docs and ADRs are updated when behavior, architecture, or workflow change
<!-- DOD:END -->
