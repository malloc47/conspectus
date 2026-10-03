---
id: CSP-496
title: Adopt effects-as-data in the reducer
status: Done
assignee: []
created_date: '2026-07-02 02:16'
labels:
  - h-tui
milestone: m-11
dependencies:
  - CSP-495
ordinal: 105000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
- Governed by ADR 0085 contract 2. Lands in phases so each wave
  ships with tests and no half-migrated state.
- Phase A (landed 2026-07-01): scaffolding + first migrations.
  `Effect` enum introduced with `Quit`, `Toast`, `Persist`, and
  `SpawnRefresh { force_local }`; `App::update` returns
  `Vec<Effect>`; `execute_effects(app, effects)` and
  `dispatch(app, msg)` land in runtime. `Msg::Quit` emits
  `Effect::Quit` (should_quit stays as state during the
  migration). Reducer contract pinned by
  `tui::app::tests::reducer_effects`. Reserved variants for
  future waves are documented in `src/tui/effect.rs` but not
  enumerated until an executor case exists.
- Phase B.1 (landed 2026-07-02): `Effect::Exec(ExecSpec)` with
  `AttachMux(AttachTarget)` and `Resume(ResumeTarget)`. Reducer
  handles `Msg::AttachSelected` / `Msg::ResumeSelected` via the
  existing pure resolvers; the executor split into pure
  `execute_effects` and live `execute_effects_live` — the latter
  is the only code that touches `&mut Terminal` /
  `std::process` for these paths. `attach_action` /
  `resume_action` free functions deleted; four reducer-level
  `(state', effects)` tests land as the first interaction-level
  coverage per ADR 0085 contract 2.
- Phase B.2 (landed 2026-07-02): `ExecSpec` grew
  `ViewSession(AgentSessionId)` and
  `LaunchPin { pin_id, attach_target }`. `Msg::ViewSelected` /
  `Msg::LaunchSelectedPin` reducer arms use pure resolvers
  (`resolve_view_session` already existed, `resolve_launch_pin`
  and `PinLaunchDisabled` moved into `tui/actions.rs` alongside
  the `PinLaunchTarget` struct). Executor's `execute_view_session`
  picks native (fs read + `open_viewer_modal`) vs external
  fallback (`claude-history`) — the reducer never touches either.
  `view_action` and `launch_pin_action` free functions deleted;
  `launch_pin_by_id` renamed to `execute_launch_pin` and now
  lives entirely under the executor. Four new reducer-level
  tests bring the `(state', effects)` coverage to 8 cases across
  all four Phase B Msgs. `&mut Terminal` no longer appears in
  any non-executor handler in the runtime.
- Phase C.1 (landed 2026-07-02, capture-pane): introduced
  `Effect::RunMux(MuxOp)` with `MuxOp::CapturePreview`.
  `execute_effects_live` and `dispatch_live` grew a `tmux`
  parameter — the executor is now the sole holder of the
  `TmuxRunner` reference for reducer-emitted effects.
  `refresh_mux_preview_if_needed` split into a pure planner
  (`plan_mux_preview_capture`) + executor branch
  (`execute_mux_op`). Pure `execute_effects` treats `RunMux` as
  a no-op the same way it does `Exec`. Four executor-level
  tests cover the CapturePreview path end-to-end with FakeTmux.
  `new-session` and `send-keys` weren't threaded through the
  runtime today so no dispatch surface to migrate.
- Phase C.2 (landed 2026-07-02 alongside Phase D.2, rename):
  the two rename call sites (`commit_rename`,
  `apply_pin_adopt_mux_rename`) migrated as bundled effects
  inside their WriteStore executor branches. Rather than
  exposing a standalone `MuxOp::RenameSession` variant, the
  executor's `execute_pin_create` and `execute_commit_alias_rename`
  call `tmux.rename_session(...)` inline after the store write,
  preserving the current composite status-message wording. A
  future story can factor the shared "rename mux by socket+
  from+to" call out into a first-class `MuxOp::RenameSession`
  once a third consumer needs it.
- Phase D.1 (landed 2026-07-02, pin remove + bind):
  `Effect::WriteStore(StoreOp)` with
  `StoreOp::PinRemove(PinRemoveRequest)` and
  `StoreOp::PinBind(PinBindRequest)`. `Msg::PinRemove` /
  `Msg::PinBind` reducer arms emit the effect (bind gates on
  the snapshot being loaded and falls back to `Effect::Toast`
  otherwise). `Effect::WriteStore` handled by
  `execute_pure_effect` — no terminal or tmux needed, so tests
  and static mode inherit the same behavior. `PinsAction::
  RemovePin` / `PinsAction::BindPin` dispatch through the new
  Msgs; the direct-key remove path preserves its confirmation
  arming as runtime orchestration but funnels the actual write
  through the reducer. `remove_pin_controls_action` and
  `bind_pin_action` free functions deleted.
- Phase D.2 (landed 2026-07-02, pin create / edit + rename
  overlay): `StoreOp` grew `PinCreate(PinCreateRequest)`,
  `PinEdit(PinEditRequest)`, and
  `CommitAliasRename { session_id, new_display_name }`.
  `Msg::PinCreate` / `Msg::PinEdit` emit their WriteStore
  directly. `Msg::CommitRename(String)` branches on selection:
  agent-session → `CommitAliasRename`; pin row → builds a
  `PinEditRequest` from `pins_context()` and emits
  `PinEdit`; anything else → `Toast`. All WriteStore variants
  consolidated under `execute_effects_live` (some need tmux
  for their bundled mux rename); pure `execute_effects` drops
  WriteStore silently, same as it does for Exec / RunMux.
  `remove_pin_action` grew terminal + tmux parameters and
  routes through `dispatch_live`; `handle_rename_overlay_key`
  similarly. Free functions `create_pin_action`,
  `edit_pin_action`, `commit_rename`, and `commit_pin_rename`
  deleted; five new reducer-level `(state', effects)` tests
  bring the phase's coverage to 19 cases.
- Phase E (landed 2026-07-05, persistence completion): the
  `Msg::SwitchView`, `Msg::SetGrouping`, `Msg::SetFilter`, and
  `Msg::SetSort` reducer arms now emit `Effect::Persist` after
  mutating state and rebuilding the tree, closing the CSP-423
  sidecar synchronization gap. Direct `self.persist_state()`
  call moved out of `App::switch_to_view` — the runtime-loop
  shutdown persist stays as a belt-and-suspenders safety net
  for future non-reducer paths that mutate state near shutdown.
  Reducer test coverage grows to 28 cases (four `emits_persist`
  variants plus a `SwitchView` no-op guard for same-view
  dispatches). Preview capture (`Effect::CapturePreview` via
  `Effect::RunMux(MuxOp::CapturePreview)`) already landed in
  Phase C.1 — the "per-tick executor sweep" wording in the
  original Phase E scope was retrospectively covered there.
- Phase F (landed 2026-07-02, collapse update layers):
  `ControlsAction` enum deleted. Four new Msg variants
  (`SwitchView`, `SetGrouping`, `SetFilter`, `SetSort`) carry
  the projection changes end-to-end; reducer arms mutate state
  and re-derive the row tree via a new
  `App::rebuild_tree_in_place()` helper. `build_tree_for_view`
  + `TreeInputs` relocated to `tui/rows/mod.rs` so the reducer
  can call them without a runtime dependency. The controls
  overlay now produces `Msg` values in
  `ControlsOutcome::ApplyAndStay(Msg)` /
  `ApplyAndClose(Msg)` per ADR 0085 contract 3. Runtime
  helpers `apply_controls_action_and_rebuild` (live + static),
  `App::apply_controls_action`, and `apply_view_switch` all
  deleted; call sites (~15 across live loop, static loop,
  controls overlay, snapshot.rs, tests) route through
  `dispatch()`/`dispatch_live()`. Reducer test coverage grows
  to 23; the existing `projection_zero_discovery` suite pins
  the invariant via the new Msg variants directly.
- Phase F cleanup (landed 2026-07-02, PinsAction collapse):
  `PinsAction` enum deleted. `PinsOutcome` carries
  `crate::tui::Msg` values directly. The four write variants
  (Create / Edit / Bind / Remove) map to the existing
  `Msg::Pin{Create,Edit,Bind,Remove}`; the launch variant
  becomes new `Msg::LaunchPinById(String)` (reducer looks up
  the attach target from the snapshot and emits
  `Effect::Exec(ExecSpec::LaunchPin { ... })`); the
  placeholder variant becomes `Msg::SetStatus(...)` with the
  hint text pre-formatted via a shared
  `pin_placeholder_status(&str)` helper. `App::apply_pins_action`
  and `apply_pins_action_and_refresh` runtime helper deleted;
  `static_apply_pins_action_and_refresh` becomes
  `static_apply_pins_msg`. Reducer test grows to 24. Scope
  note: the eight `Action::Open*` overlay-opens still live in
  the Action enum — they fold naturally when `CSP-497`'s
  modal stack lands with a shared Open contract.
- Blockers: `CSP-495` (landed). Phase B unblocks after
  Phase A; C/D/E/F land opportunistically as their variants
  are needed.
<!-- SECTION:DESCRIPTION:END -->

Legacy ID: `H-TUI-002`
