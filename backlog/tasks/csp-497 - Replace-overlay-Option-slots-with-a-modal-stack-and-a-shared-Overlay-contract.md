---
id: CSP-497
title: Replace overlay Option slots with a modal stack and a shared Overlay contract
status: Done
assignee: []
created_date: '2026-07-02 02:16'
labels:
  - h-tui
milestone: m-11
dependencies:
  - CSP-496
ordinal: 106000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
- Scope: eight modal surfaces (seven `Option` fields on `App` plus the
  viewer island) become an explicit `Vec<Modal>`: input routes to the
  stack top, draw renders in stack order, Esc/commit pops. One
  `Overlay` trait (`handle(&mut self, KeyEvent) -> OverlayOutcome`
  with `Consumed` / `Commit(Msg)` / `Close`, plus
  `render(frame, area, &Theme)`) implemented by the existing widgets;
  the viewer becomes a nested-reducer stack entry
  (`Msg::Viewer(ViewerMsg)`). Adding a modal becomes struct + trait
  impl + enum variant with zero new loop/keymap/draw branches. The
  `CSP-470` pins-widget split should target this contract.
- Wave 1 (landed 2026-07-02, scaffolding + Help): new `tui/modal.rs`
  with `Overlay` trait, `OverlayOutcome { Consumed, Commit(Box<Msg>),
  Close }`, and `Modal` enum. `App::modal_stack: Vec<Modal>` replaces
  the `help_overlay: Option<...>` field for the first migrated
  surface. `open_help_overlay` / `close_help_overlay` /
  `help_overlay()` / `help_overlay_mut()` route through the stack so
  every runtime + UI caller keeps working. `handle_help_overlay_key`
  dispatches through the `Overlay::handle` trait method; `Commit`
  outcomes pop + `dispatch()` through the reducer.
- Wave 3 (landed 2026-07-02, Controls migration): `Modal::Controls`
  variant + storage migration for the controls overlay. Widget
  doesn't implement the `Overlay` trait because its `handle_key`
  reads a live `ControlsContext` borrowed from `App`; a future
  story either grows the trait with an associated context type
  or refactors the widget. Runtime's existing dispatch stays.
  Multi-overlay stacking now works — Help can open on top of
  Controls and pop back to it. Test count grows from 5 to 10 in
  `tui::app::tests::modal_stack` with stacking + guard-against-
  naive-pop coverage.
- Wave 4 (landed 2026-07-02, Pins migration): `Modal::Pins`
  variant + storage migration. Same shape as Controls — widget
  doesn't implement `Overlay` because handle_key reads a
  `PinsContext` at event time. `set_pins_overlay` (direct-key
  openers with pre-configured state) also migrated to push
  onto the stack. Triple-stack scenarios exercised via
  `triple_stack_orders_correctly_and_pops_lifo`: Controls at
  bottom, Pins in middle, Help on top; pop reveals each in
  turn. Test count grows from 10 to 14.
- Wave 5 (landed 2026-07-02, rename + search + value_modal
  rolled together): three simple overlays migrate in one
  landing. `Modal::Rename` / `Modal::Search` /
  `Modal::ValueModal` variants added; corresponding Option
  fields deleted; accessors + open/close methods migrated to
  the stack. `ValueModalState` gets an `Overlay` trait impl
  (its Continue/Close outcome maps cleanly to Consumed/Close);
  `handle_value_modal_key` routes through the trait's
  handle()/OverlayOutcome path, matching help's pattern.
  Rename and Search stay with their specialized handlers —
  their Confirm outcomes carry values that map to specific
  Msgs (Msg::CommitRename, App::set_selection) at the call
  site, and moving the mapping into a widget-side Msg
  constructor would couple the widgets to the reducer's
  enum. Test count grows from 14 to 21, including a five-
  wide LIFO stack test with Controls / Pins / Search /
  Rename / Help.
- Wave 7 (landed 2026-07-02, viewer_modal): the transcript
  viewer migrated as a nested-reducer stack entry per ADR
  0085 contract 3. New `Msg::Viewer(ViewerMsg)` variant; the
  reducer arm pops the top `Modal::Viewer` state, runs it
  through `viewer::input::reduce`, then pushes the new state
  back on `ViewerEffect::None` or leaves it popped + sets
  "viewer closed" status on `ViewerEffect::Close`. The
  take-reduce-put ownership dance moves from the runtime helper
  into the reducer arm; `take_viewer_modal` accessor deleted.
  `handle_viewer_overlay_key` now translates keys to
  `ViewerMsg` and dispatches through the shared
  `dispatch(app, Msg::Viewer(vmsg))`. First Elm/Bubble Tea
  nested-reducer example in the codebase. Test count grows
  from 21 to 26, including a "Help-on-top-of-Viewer keeps
  Viewer state exactly as-is" guard.
- Landing state: every overlay is on the modal stack. Help
  and ValueModal implement the `Overlay` trait. Viewer uses
  the nested-reducer composition. Controls, Pins, Rename, and
  Search stay with specialized dispatchers — Controls and
  Pins because their handlers need widget-side context;
  Rename and Search because their `Confirm` outcomes carry
  values that the runtime maps to specific Msgs at the call
  site. Growing the `Overlay` trait with an associated
  context type (or refactoring those widgets to internalize
  their context) is follow-up territory and doesn't block
  other H-TUI-* work.
- Tests: overlay snapshot tests unchanged; one stacking test (e.g.
  help over controls) and a routing test per outcome variant.
- Blockers: `CSP-496` (`Commit(Msg)` needs the unified Msg/Effect
  path) landed. Wave 1 landed. Subsequent waves are mechanical.
<!-- SECTION:DESCRIPTION:END -->

Legacy ID: `H-TUI-003`
