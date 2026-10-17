---
id: CSP-617
title: Fix `pin adopt` so it finds live tmux sessions
status: Done
assignee:
  - '@claude'
created_date: '2026-10-17 03:09'
updated_date: '2026-10-17 23:46'
labels:
  - h-ext
milestone: m-11
dependencies: []
references:
  - docs/code-hygiene-audit.md
priority: high
type: bug
ordinal: 675000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
`conspectus pin adopt <id> <mux-name>` fails for every live tmux session. It builds the prefixed id form (`tmux:<name>`, or `tmux:<socket>:<name>`) and compares it with `MuxSessionNode.native_id` (`src/cli/pin.rs:635-645`), but discovery stores the bare session name in that field and keeps the prefixed form on `MuxSessionNode.id` (`src/discovery/tmux/mod.rs:1558-1560`). Reproduced on 2026-10-16 against an isolated tmux server: with session `adoptme` running, discovery emitted `id = tmux:adoptme, native_id = adoptme`, and `conspectus pin adopt verify-pin adoptme --harness codex --cwd <dir>` exited 1 with "no live mux with native_id `tmux:adoptme`". The only adopt integration test (`tests/cli_pin.rs::pin_adopt_fails_when_mux_not_live`) covers the failure path, so nothing exercised a successful adopt against nodes shaped the way discovery emits them. Adopt is the documented way to bring an existing tmux session under a pin (ADR 0057).

- Scope: match the requested mux against the field that carries the prefixed form, for default and named sockets, and leave the stored pin's `mux` fields unchanged. The TUI adopt flow takes a different code path; check that it applies the same matching rule.
- Tests: a CLI-level test that adopts a mux whose node has the shape discovery emits (bare `native_id`, prefixed `id`) and asserts the pin is written; keep the not-live failure test.
- Manual checks: against a scratch tmux server (`TMUX_TMPDIR` pointed at a short temp directory, `HOME` and the XDG dirs at a temp tree), run `tmux new -d -s adoptme`, then `conspectus pin adopt p adoptme --harness codex --cwd <dir>` writes the pin.
- Blockers: none.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [x] #1 Running `conspectus pin adopt` against a live tmux session on the default socket writes the pin
- [x] #2 A test covers a successful adopt against mux nodes shaped the way discovery emits them
- [x] #3 The CLI and TUI adopt paths apply the same mux matching rule
<!-- AC:END -->

## Definition of Done
<!-- DOD:BEGIN -->
- [x] #1 `just check` passes, or `git diff --check` for docs-only changes
- [x] #2 Docs and ADRs are updated when behavior, architecture, or workflow change
<!-- DOD:END -->

## Implementation Plan

<!-- SECTION:PLAN:BEGIN -->
1. Expose the resolver's pin-to-mux lookup (`resolve::pins`) as a shared function: given the snapshot nodes and a `PinMux`, return the live `MuxSessionNode` keyed exactly as `mux_index` keys it.
2. In `PinAdoptArgs::run`, build the `PinMux` (tmux backend, name, socket) first and find the mux through that function instead of comparing the prefixed id against `MuxSessionNode.native_id`. Reuse the same `PinMux` for the written entry.
3. Make the not-found error say when a non-default socket is the reason (discovery only enumerates the default socket).
4. Add a CLI test in `tests/cli_pin.rs` using `PrivateTmux`: create a real detached session, run `pin adopt` with tmux discovery enabled, assert success and the written pin entry; keep the not-live failure test.
5. TUI adopt takes the selected mux row and writes its bare `native_id` as the pin's mux name, which is the same rule the resolver binds on; confirm with the resolver lookup in a unit test.
<!-- SECTION:PLAN:END -->

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
- `resolve::pins` now exposes `live_mux_for_pin_key`, built on the same `pin_key_for_mux` key `mux_index` uses for binding, so adopt and binding cannot disagree. CLI adopt builds its `PinMux` first and looks the mux up by `PinMux::native_id()`.
- TUI adopt takes the selected mux row and writes its bare `native_id` as `mux.name`, which produces the same key; `tui::app::tests::pin_adopt_defaults_preserve_selected_live_mux_name` and the new `live_mux_for_pin_key_finds_the_mux_a_pin_binds_to` cover the two halves.
- Found while working: the TUI pin create/adopt paths (`tui/runtime/pin_store.rs:188,294`) always write `backend = tmux`, so adopting a zellij row from the TUI writes a tmux pin. Outside this task; flagged for the operator.
- Validation: the new `tests/cli_pin.rs::pin_adopt_writes_a_pin_for_a_live_tmux_session_from_discovery` fails on the old code with the original error and passes with the fix; `just check` green (2391 tests).
<!-- SECTION:NOTES:END -->

## Final Summary

<!-- SECTION:FINAL_SUMMARY:BEGIN -->
`conspectus pin adopt` now finds live tmux sessions. It looked the mux up by comparing the prefixed id (`tmux:<name>`) with `MuxSessionNode.native_id`, which discovery stores bare, so it could never match. It now uses `resolve::pins::live_mux_for_pin_key`, the same key pin binding uses, and the not-found error explains when a non-default socket is the reason. Verified with a new CLI test on a private tmux server (fails before, passes after), a resolver unit test, and `just check` (2391 tests).
<!-- SECTION:FINAL_SUMMARY:END -->
