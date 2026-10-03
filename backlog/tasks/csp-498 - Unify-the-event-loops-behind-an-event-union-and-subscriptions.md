---
id: CSP-498
title: Unify the event loops behind an event union and subscriptions
status: Done
assignee: []
created_date: '2026-07-02 02:16'
labels:
  - h-tui
milestone: m-11
dependencies:
  - CSP-496
ordinal: 107000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
- Landed across waves 1 + 2 + 3-keymap on 2026-07-02.
  `UiEvent` scaffolding + shared helpers (wave 1),
  `LoopMode` trait + `run_loop` shared driver + `LiveMode` /
  `StaticMode` impls (wave 2), and the keymap module
  extraction (wave 3) all shipped. Further runtime.rs
  module split (loop_driver + effect_executor) is
  deferred as optional cleanup — it requires ~30 helper
  visibility changes for a purely mechanical move with no
  architectural or behavioral benefit, and is best folded
  into `CSP-470` if that story activates.
- Scope: `enum UiEvent { Input(Event), Tick, Discovery(..) }` consumed
  by one loop parameterized by its subscription set — live mode
  subscribes to the discovery channel and refresh timer, fixture mode
  to file reload, snapshot mode runs the body once. Absorbs
  `CSP-469`'s dual-loop unification; then split `runtime.rs` into
  loop driver / keymap / effect executor modules.
- Wave 1 (landed 2026-07-02, shared helpers + UiEvent
  scaffolding): introduced `UiEvent { Input(Event), Tick,
  Discovery(DiscoveryResult) }` enum (dead-code allowed until
  a future wave wires it into a `next_ui_event` stream).
  Extracted two shared helpers both loops now use: `draw_frame(app,
  terminal)` (the toast prep + viewer-or-ui render block that
  was byte-identical in both loops) and
  `overlay_key_from_event(app, event)` (the modal-stack-aware
  input routing that returns `Some(Action::*OverlayKey(key))`
  or `None` for the caller's per-mode fallback). Dedupes ~160
  lines from runtime.rs.
- Wave 2 (landed 2026-07-02, unify loop bodies): introduced
  `trait LoopMode { init, drain, dispatch, tmux }` with
  `LiveMode` and `StaticMode` impls. New `fn run_loop(terminal,
  app, config, mode)` shared driver: mode.init → while
  !should_quit → draw_frame → mode.drain → event::poll →
  overlay_key_from_event.or_else(translate) → mode.dispatch →
  refresh_mux_preview → app.persist_state. `event_loop` and
  `static_event_loop` shrink to ~15-line wrappers that
  construct their mode and call run_loop. Discovery channel
  + refresh timer moved into `LiveMode`; fixture reload moved
  into `StaticMode`. Net diff: +445 / -403 (the dispatch
  matches are still mode-specific inside trait impls, not
  deduped; the loop skeleton, event poll, overlay routing,
  draw pipeline, and mux-preview refresh are all shared).
- Wave 3 (partial, landed 2026-07-02, keymap extracted):
  `tui/keymap.rs` module created with Action / translate /
  remap_for_focus / cycle_view / SelectedDefault /
  selected_default_action. runtime.rs shrinks by ~370 lines
  and re-exports these via `pub(super) use crate::tui::keymap`
  so the ~1000-line test module in runtime.rs keeps working
  unchanged. Loop driver and effect executor stay in
  runtime.rs; extracting them requires cascading visibility
  changes across ~30 runtime helpers referenced by LoopMode
  impls and is deferred as an optional follow-up.
- Tests: live, fixture, and ADR 0067 snapshot suites unchanged.
- Blockers: `CSP-496` (landed). Waves 1 + 2 landed. Wave 3
  is optional cleanup and can happen alongside `CSP-470`
  (which also touches runtime.rs boundaries).
<!-- SECTION:DESCRIPTION:END -->

Legacy ID: `H-TUI-004`
