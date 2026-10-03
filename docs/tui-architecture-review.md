# TUI Architecture Review

Status: draft review, 2026-07-01. A fresh-eyes read of the TUI as
implemented today — deliberately ignoring prior ADRs and design history per
the review request — evaluated against the common interactive-UI
architectures, with recommendations for making further TUI development
smooth. Companion to the code-level findings in
`docs/code-hygiene-audit.md`; this document is about the *shape* of the
TUI, not line-level hygiene.

## The Architecture As Implemented

**Data flow.** A resolved `GraphSnapshot` (wrapped as
`GraphDb(Rc<GraphSnapshot>)`, `app.rs:41`) is the immutable input. Row
builders (`tui/rows/*`) project it into a `RowTree` view-model per view;
`detail.rs`/`explorer.rs` project the current selection into a
`NodeDetail`/`ExplorerState`. Rendering is immediate-mode ratatui:
`ui::draw(&App, frame)` reads `App` and paints everything each frame,
overlays last in a fixed stacking order (`ui.rs:59`).

**State.** One central `App` struct (`app.rs:100`, ~45 fields) holds the
snapshot, the row tree, selection/expansion, per-view state slots
(`ViewStateSlot`), the detail/explorer projections, provider status, a
preview cache — and seven independent overlay slots
(`rename_overlay`, `controls_overlay`, `pins_overlay`, `search_overlay`,
`help_overlay`, `value_modal`, `viewer_modal`) plus armed-flag scalars
(`pending_pin_remove`, `explorer_back_armed`) and a toast engine.

**Update.** Four accreted layers:

1. **`Msg` + `App::update`** (`app.rs:382`) — a genuine reducer for pure
   state transitions (navigation, selection, `SetData`, toggles). Pure,
   synchronous, well-tested.
2. **`Action`** (`runtime.rs:1353`) — the keymap's output. `translate`
   (pure crossterm→`Action` map) then `remap_for_focus` (focus modality).
   `Action::Msg(..)` funnels to the reducer; a dozen effectful variants
   (`Attach`, `Refresh`, `Resume`, `LaunchPin`, …) dispatch to free
   functions that take `(terminal, app, config, tmux)` and perform side
   effects inline.
3. **Per-overlay key pipelines** — when an overlay is open, the keymap
   wraps the raw key as `RenameOverlayKey(KeyEvent)` /
   `ControlsOverlayKey` / `PinsOverlayKey` / `SearchOverlayKey` /
   `HelpOverlayKey` / `ValueModalKey`, and a per-overlay
   `handle_*_overlay_key` in the runtime drives that overlay's private
   state machine, interpreting its committed result (`PinsAction`,
   `ControlsAction`, …) with more inline effects.
4. **The viewer island** — the transcript viewer is its own complete
   Elm-style unit (`ViewerState` + `ViewerMsg` + reducer + keymap at
   `runtime.rs:2339`), bolted on as an eighth modal slot.

**Effects.** Split personality. The periodic/`r` refresh is properly
asynchronous (`spawn_discovery_worker` → mpsc → `Msg::SetData`). Everything
else is synchronous inside input handling: pin CRUD shells out to
`conspectus pin …` and then runs `refresh_after_pin_mutation` — a full
discovery pass — on the UI thread; attach/view/launch suspend raw mode and
exec subprocesses with `&mut DefaultTerminal` threaded deep into handlers;
mux preview capture runs after each action in the loop.

**Loops.** Two near-identical event loops (`event_loop`,
`static_event_loop`) differing only in refresh source, plus a third
distinct entry for snapshot mode.

**One load-bearing detail** (`runtime.rs`, `apply_controls_action_and_refresh`):

```rust
app.apply_controls_action(action);
refresh(app, config);           // synchronous discover_and_build()
```

Every view switch, grouping change, and filter change re-runs discovery
synchronously — daemon IPC at best, full cold discovery (including `gh`)
at worst — even though `App.database` already holds the snapshot and the
change only alters the *projection*. Relatedly, `build_tree_for_view`
reads `config.default_view` / `config.sessions_grouping`, so `RunConfig`
doubles as live UI state that must be kept in sync with `App`'s per-view
slots — double bookkeeping between config-as-input and config-as-state.

## What Model Is This?

Measured against the standard interactive-UI architectures:

- **Elm architecture / MVU (Elm, Bubble Tea, Redux+middleware):** this
  codebase is ~70% of the way there already — single state value,
  message enum, reducer, immediate-mode view as a pure function of state,
  async results delivered as messages. What's missing is the other 30%:
  *one* message type instead of four update layers, and *effects as
  data* instead of effects inlined into input handlers.
- **Component-tree frameworks (tui-realm, React-style):** poor fit. The
  UI is two panes plus modals over one shared graph — there is no deep
  component hierarchy with local state to justify component identity,
  props plumbing, or a framework dependency.
- **Retained-mode / reactive (Dioxus TUI, iocraft):** no. Immediate mode
  over a single state value is the right regime for this UI and is
  already snapshot-testable (`--snapshot` harness).
- **Modal state machine for input (vim/kakoune-style mode stack):** yes —
  this is the missing model for the overlay layer specifically. The
  seven `Option` slots plus armed flags are an implicit mode system whose
  exclusivity and priority are encoded in if-else order in two event
  loops and in the draw-call order.

Verdict: **don't adopt a framework; finish the Elm shape the code is
already converging on.** Every pain point below is a place where the
implemented code deviates from the model it's clearly reaching for.

## Findings

- **F1 — Effects are interleaved with input handling.** Handlers take
  `(terminal, app, config, tmux)` and perform shell-outs, execs, TOML
  writes, and refreshes mid-keystroke. Consequences: the dispatch layer
  is untestable without a terminal, the two event loops must duplicate
  the entire effectful match, and effect ordering (e.g. rename → toast →
  refresh → reselect) is implicit in statement order per call site.
- **F2 — Projection changes re-fetch the world.** View/grouping/filter
  switches call synchronous `refresh` instead of re-deriving `RowTree`
  from the held snapshot. This is both a responsiveness cliff (blocking
  the render loop on discovery) and a conceptual inversion: selectors
  should be derived state, cheap to recompute from `(snapshot, view,
  grouping, filter, sort)`.
- **F3 — `RunConfig` doubles as live UI state.** The tree builder reads
  view/grouping from config, so runtime code clones and mutates config
  copies to reflect App state before rebuilding. Two sources of truth
  for the same facts.
- **F4 — Modality is implicit.** Eight modal surfaces (seven overlays +
  viewer) as independent `Option` fields, with mutual exclusivity
  enforced only by keymap wrapping order and draw order. Adding a ninth
  modal touches: App field, keymap wrap variant, event-loop branch (×2
  loops), draw call, and focus rules — none of which the compiler
  connects.
- **F5 — The renderer mutates state.** `left_scroll` / `explorer_scroll` /
  `last_visible_index` are `Cell`s so `draw(&App)` can adjust scroll
  during layout. Scroll reconciliation is state logic that happens to
  need viewport measurements; hiding it in interior mutability makes
  draw non-pure and ordering-sensitive.
- **F6 — Loop duplication.** Two event loops share toast ticking, poll
  timeout, dispatch, and render, differing only in where refreshes come
  from. The viewer island additionally runs its own key handling path.
- **F7 — Overlays are seven hand-rolled proto-components.** Each has
  state + widget + key handling + commit type, but each invents its own
  contract, its own centered-rect math, and its own runtime handler.
  They are one trait away from being uniform.

None of this is exotic damage — it is exactly the residue of building
each feature as a vertical slice without a declared UI architecture to
slot into. The foundations (reducer, immutable snapshot, view-models,
immediate mode, async discovery, snapshot tests) are the right ones.

## Recommendations

Target model, in one paragraph: **a single-store Elm loop with effects as
data, a modal stack, and derived view-models.** One `Msg` enum; one
`update(&mut App, Msg) -> Vec<Effect>`; one runtime that turns events into
messages, runs the reducer, executes effects (owning the terminal, the
mux runner, and worker spawning), and draws. Modals are components on an
explicit stack. Row trees and detail views are memoized derivations of
App state.

Concretely, in landing order:

- **R1 — Derived view-models first** (fixes F2/F3, biggest UX win).
  Make `RowTree` a derivation: `derive_tree(snapshot, view, grouping,
  filter, sort, now)` computed from App fields, recomputed when any
  input changes (a dirty flag or a cache key tuple is sufficient — no
  reactive machinery needed). View/grouping/filter changes become pure
  `Msg`s that never touch discovery; only `r`, the timer, and pin/alias
  mutations schedule refreshes. Delete the config-mutation pattern;
  `RunConfig` becomes initial-values-only. This pairs with the planned
  `SnapshotIndex` (`CSP-467`) as the shared substrate the builders
  read.
- **R2 — Effects as data** (fixes F1). Collapse `Action`'s effectful
  variants and the per-overlay commit handling into the reducer, and
  give it a return channel: `enum Effect { SpawnRefresh { force_local:
  bool }, RunMux(MuxOp), Exec(ExecSpec), WriteStore(StoreOp), CapturePreview(MuxTarget),
  Toast(..), Persist(..), Quit }`. The runtime owns an effect executor —
  the only code that sees `&mut Terminal`, the mux runner, and
  `std::process`. Long-running effects complete by sending a `Msg` back
  (the discovery worker already models this). Reducer tests then cover
  every interaction end-to-end by asserting `(state', effects)` — no
  terminal, no tmux, no fs.
- **R3 — Explicit modal stack** (fixes F4). `Vec<Modal>` where `enum
  Modal { Controls(..), Pins(..), Search(..), Rename(..), Help,
  Value(..), Viewer(..) }`. Input routes to the top of the stack first;
  `draw` renders the stack in order; `Esc`/commit pops. Exclusivity,
  priority, and "suspend the underlying keymap" become structural
  instead of conventional. The viewer stops being a special eighth slot
  and becomes a stack entry whose messages nest (`Msg::Viewer(ViewerMsg)`
  — nested reducers are the standard Elm/Bubble Tea composition and the
  viewer already has exactly that shape).
- **R4 — One `Overlay` component contract** (fixes F7). A small trait —
  `handle(&mut self, KeyEvent) -> OverlayOutcome` (`Consumed`,
  `Commit(Msg)`, `Close`) plus `render(&self, frame, area, &Theme)` —
  implemented by the seven existing widgets. New modals become a state
  struct + trait impl + one `Modal` variant; the keymap, loops, and draw
  need zero new branches.
- **R5 — One loop, event union, subscriptions** (fixes F6). `enum
  UiEvent { Input(Event), Tick, Discovery(DiscoveryResult) }` consumed by
  a single loop parameterized by its subscription set: live mode
  subscribes to the discovery channel and the refresh timer; fixture
  mode subscribes to file reload; snapshot mode runs the loop body once.
  This subsumes the dual-loop unification already filed as `CSP-469`.
- **R6 — Scroll reconciliation into update** (fixes F5). Emit viewport
  dimensions with input (`PageDown(u16)` already does this) or as a
  post-layout `Msg::ViewportChanged`, reconcile scroll in the reducer,
  and drop the `Cell` fields so `draw` is strictly `&App → pixels`.
- **R7 — Declarative keymap on top.** Once R2/R3 exist, the binding
  table already filed as `CSP-468` becomes trivial: `(mode, key) →
  Msg` rows, with the modal stack supplying the mode. Help text, hint
  footers, and dispatch all read the same table.

**Explicit non-recommendations:** no `tui-realm` or component framework
(the UI shape doesn't need it and the dependency would fight the
snapshot-test harness); no retained-mode rewrite; no actor/channel
architecture between panes (one store is correct at this scale); no
attempt to make the reducer `async` (effects-as-data keeps it sync and
testable).

## Relationship To Filed Work

Filed in `docs/backlog.md` § TUI Architecture Convergence: R1 →
`CSP-495`, R2 → `CSP-496`, R3+R4 → `CSP-497`, R5 → `CSP-498`,
R6 → `CSP-499`. R7 stays `CSP-468` (keybinding table), which the
`CSP-497` modal stack later supplies with its mode column.

The review refines rather than replaces the previously filed stories:
`CSP-495` builds on `CSP-467` (SnapshotIndex), extending "index the
snapshot" to "make trees derived state"; `CSP-498` absorbs
`CSP-469` (dual event loops — closed as folded); `CSP-497` gives
`CSP-470`'s pins-widget split a contract to split *toward*. R2
(effects as data, `CSP-496`) is the one genuinely new architectural
commitment — it is also the piece that makes the rest cheap, and owes
an ADR when adopted since it changes how every future TUI feature is
written.

Suggested sequencing if adopted: `CSP-495` → `CSP-496` →
`CSP-497` → `CSP-498` → `CSP-499`/`CSP-468` opportunistically.
`CSP-495` stands alone and pays immediately (no discovery on view
switches); `CSP-496` is the enabling investment; everything after is
mechanical once it exists.
