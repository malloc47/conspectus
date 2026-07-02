# ADR 0085: TUI Elm/MVU Architecture

## Status

Accepted

## Context

The TUI has grown from a small ratatui prototype into the primary
Conspectus interaction surface (Phase 8, plus the pin, detail, and
viewer follow-ups). A fresh-eyes review recorded in
`docs/tui-architecture-review.md` (2026-07-01) found that the code is
already ~70% of an Elm/MVU shape — a single `App` state value, a
`Msg` enum, an `App::update` reducer, immediate-mode ratatui rendering
as a pure function of state, and asynchronous discovery delivered back
as messages — but that the remaining 30% has accreted into four
overlapping "update layers" and a set of ad-hoc modality and effect
patterns:

- Projection changes (view/grouping/filter switches) re-run synchronous
  discovery on the UI thread instead of re-deriving `RowTree` from the
  held snapshot, and `RunConfig` doubles as live UI state that the
  runtime clones and mutates.
- Effects (subprocess exec, mux ops, TOML store writes, refresh
  scheduling, preview capture) are interleaved with input handling.
  Handlers take `(terminal, app, config, tmux)` and shell out
  mid-keystroke, so the dispatch layer is untestable without a
  terminal and both event loops must duplicate the effectful match.
- Eight modal surfaces (seven `Option` overlay slots on `App` plus the
  viewer island) enforce mutual exclusivity by keymap-wrap order and
  draw order alone. Adding a modal touches an `App` field, a keymap
  wrap variant, two event-loop branches, a draw call, and focus
  rules — none of which the compiler connects.
- Two near-identical event loops (`event_loop`, `static_event_loop`)
  plus a third snapshot-mode entry differ only in their refresh
  source. The viewer additionally runs its own key-handling path.
- The renderer mutates `Cell` fields (`left_scroll`,
  `explorer_scroll`, `last_visible_index`) during `draw` to reconcile
  scroll against layout, so `draw(&App)` is not strictly pure.

The review evaluated the standard interactive-UI architectures against
the code and concluded that the fit is unambiguous:
component-tree frameworks (`tui-realm`, React-style) do not repay their
dependency cost on a two-pane-plus-modals UI over a single graph;
retained-mode / reactive systems (Dioxus TUI, iocraft) are the wrong
regime for a snapshot-testable single-store UI (ADR 0067);
modal-state-machine input models (vim / kakoune) address only the
overlay layer specifically. The Elm architecture — a single-store
reducer + immediate-mode view + effects-as-data — is the shape the
codebase is already converging on, and every pain point above is a
place where the implementation deviates from that model rather than a
sign that the model is wrong.

The related audits filed the concrete cleanup as `H-TUI-001..005`
(architecture convergence) and share substrate with `H-HYG-006`
(`SnapshotIndex`), `H-HYG-007` (declarative keybinding table), and
`H-HYG-009` (TUI monolith splits). What was missing was a written
guardrail so that (a) those stories land against a shared target
picture and (b) future TUI feature work does not re-introduce the
same drift by building each feature as another vertical slice with
its own effect handling and its own modal contract.

## Decision

Conspectus's TUI is a **single-store Elm/MVU loop with effects as
data, a modal stack, and derived view-models.** All future TUI work
respects this shape; deviations are the audit signal to refactor,
not the precedent to extend.

The target model is five contracts:

### 1. One `Msg`, one `update`, one `App`

`App` is the sole owner of TUI state. All state transitions happen in
`update(&mut App, Msg) -> Vec<Effect>`, which is pure, synchronous,
and testable without a terminal, a mux runner, or the filesystem.
`Msg` is a single enum covering navigation, selection, overlay
open/commit/close, discovery deliveries, viewport reports, and any
future feature's inputs; the four historical update layers (`Msg` +
`App::update`, `Action` variants, per-overlay commit types, viewer
reducer) collapse into it. Nested reducers compose via
`Msg::<Sub>(SubMsg)` in the standard Elm/Bubble Tea shape (the viewer
already has exactly this form and is the reference example).

### 2. Effects as data

The reducer emits an `Effect` value for every side effect it wants to
schedule: `SpawnRefresh`, `RunMux(MuxOp)`, `Exec(ExecSpec)`,
`WriteStore(StoreOp)`, `CapturePreview(MuxTarget)`, `Toast(..)`,
`Persist(..)`, `Quit`, and so on. The runtime owns a single effect
executor — the only code in the TUI that touches `&mut Terminal`, the
mux runner, `std::process`, or `fs`. Long-running effects complete by
sending a `Msg` back on the discovery channel; the reducer never
blocks on I/O.

Reducer tests then cover every interaction end-to-end by asserting
`(state', effects)` against a fixture — no terminal, no tmux, no
filesystem. Effect ordering per interaction becomes explicit data
instead of implicit statement order per call site.

### 3. Modal stack + one `Overlay` contract

Modality is structural. `App` holds a `Vec<Modal>`; input routes to
the top of the stack first; `draw` renders the stack in order; commit
and close pop. Every overlay implements one trait:

```rust
trait Overlay {
    fn handle(&mut self, key: KeyEvent) -> OverlayOutcome;
    fn render(&self, frame: &mut Frame, area: Rect, theme: &Theme);
}

enum OverlayOutcome { Consumed, Commit(Msg), Close }
```

Adding a new modal is a state struct + trait impl + one `Modal`
variant. The keymap, loops, and draw acquire zero new branches. The
viewer stops being an eighth special slot and becomes a stack entry
with a nested reducer per contract 1.

### 4. Derived view-models

`RowTree`, `NodeDetail`, and `ExplorerState` are pure derivations of
`(snapshot, view, grouping, filter, sort, now, selection)`. Row-tree
recomputation happens when any input changes — a cache key tuple or
dirty flag is sufficient — and never triggers discovery. Only `r`,
the refresh timer, and store-mutation effects schedule refreshes.
`RunConfig` reverts to initial-values-only; per-view state and the
active view live on `App`. The `SnapshotIndex` from `H-HYG-006` is
the shared substrate the derivations read.

### 5. One event loop, one draw contract

A single `UiEvent { Input(Event), Tick, Discovery(DiscoveryResult), … }`
union is consumed by one loop parameterized by its subscription set:
live mode subscribes to the discovery channel and the refresh timer,
fixture mode to file reload, snapshot mode runs the body once. The
dual event loops and the viewer's private key-handling path fold
into this loop.

`draw(&App, &mut Frame)` is strictly `&App → pixels`. Scroll
reconciliation runs in the reducer against viewport dimensions
delivered as `Msg` — the `PageDown(u16)` pattern generalized to a
`Msg::ViewportChanged { pane, rect }` post-layout report or an
input-side viewport annotation. The `Cell` scroll fields go away.

### Non-goals

- **No component framework.** `tui-realm`, React-style component
  trees, and similar retained-mode systems are explicitly rejected.
  The UI is two panes plus modals over one shared graph — the
  hierarchy that would justify them does not exist. A framework
  dependency would also fight the snapshot-test harness (ADR 0067).
- **No retained-mode / reactive rewrite.** Immediate-mode over a
  single state value is correct at this scale and is what the
  snapshot regression net (ADR 0067, 0068, 0069) is built on.
- **No async reducer.** Effects-as-data keeps `update` synchronous
  and testable; async lives entirely in the effect executor and the
  worker channel.
- **No actor / channel architecture between panes.** One store is
  correct at this scale; the panes are views over the same state,
  not independent processes.

## Consequences

**For future TUI features:**

Every new feature is expressible as: new `Msg` variants, new
`update` arms, new `Effect` variants if it needs a side effect the
executor does not already provide, new `Modal` + `Overlay` impl if
it opens a modal surface. The four update layers, the terminal-in-
handler pattern, the `Option`-slot overlay pattern, and the
projection-triggers-discovery pattern are all off-charter.

Code reviews on TUI changes gate on these contracts. Introducing a
new `Option<FooOverlay>` field on `App`, a new `Action` variant that
executes side effects inline, a new synchronous `refresh` call from
inside input handling, or a new event-loop copy is the audit signal
to refactor, not to accept.

**For the filed stories:**

- `H-TUI-001` (derived row trees) → contract 4.
- `H-TUI-002` (effects as data) → contract 2. This is the enabling
  investment; landing it makes the rest mechanical.
- `H-TUI-003` (modal stack + `Overlay` trait) → contract 3.
- `H-TUI-004` (event union + subscriptions, subsumes `H-HYG-008`) →
  contract 5.
- `H-TUI-005` (scroll reconciliation into reducer) → the last
  fragment of contract 1 (draw purity).
- `H-HYG-006` (`SnapshotIndex`) is the substrate contract 4 reads.
- `H-HYG-007` (declarative keybinding table) becomes trivial once
  contracts 2 + 3 exist: `(mode, key) → Msg` rows with the modal
  stack supplying the mode column.
- `H-HYG-009` (TUI monolith splits) targets the `Overlay` contract
  as its split boundary.

Suggested sequencing: `H-TUI-001` → `H-TUI-002` → `H-TUI-003` →
`H-TUI-004` → `H-TUI-005`/`H-HYG-007` opportunistically. `H-TUI-001`
stands alone and pays immediately (no discovery on view switches).

**For the snapshot regression net:**

The immediate-mode + pure-`draw` shape is what makes ADR 0067's
`--snapshot` harness a coherent regression signal. Contract 5's
draw-purity rule preserves that; contract 2's reducer-testable shape
extends the regression net to interaction-level coverage without a
terminal in the loop.

**For `RunConfig` and persisted state:**

`RunConfig` is the initial-values source only. Per-view state
(active view, grouping, filter, sort, per-view selection) lives on
`App` and is persisted via `Effect::Persist` writes routed through
the executor. The current pattern of cloning `RunConfig` inside
`refresh` to reflect App state disappears.

**For non-live modes:**

Fixture mode (ADR 0068 / ADR 0069) and snapshot mode (ADR 0067)
share the loop from contract 5 and differ only in their subscription
set. This keeps the fixture and snapshot regression paths tracking
the live path structurally, not by convention.

## Alternatives Considered

**Adopt `tui-realm` (or a similar TUI component framework).**
Rejected. The UI does not have a component hierarchy that repays the
framework's abstractions — every "component" would be either the
row tree, the detail pane, or a modal, all of which read from the
same graph store. The framework would also fight ADR 0067's
snapshot mode (its own render abstractions get in the way of driving
a single frame to stdout) and force a re-architecture of the
existing reducer for no user-visible benefit. The review recorded
this as an explicit non-recommendation.

**Retained-mode / reactive TUI (Dioxus TUI, iocraft).** Rejected.
Immediate-mode rendering over a single state value is the correct
regime for a snapshot-testable UI at this scale. The reactive
machinery would replace a working, well-covered render path with a
dependency and a paradigm shift for no operator-visible win.

**Actor / channel architecture between panes.** Rejected. One store
is correct here — the panes are views over the same graph snapshot,
not independent processes. Actor-per-pane would re-introduce the
projection-triggers-discovery pattern by giving each pane its own
"fetch when I change" reflex.

**Async reducer (`async fn update`).** Rejected. Effects-as-data
gives the same expressiveness with a synchronous, testable reducer;
async lives in the effect executor and the worker channel where it
already belongs. The reducer must stay a plain function of
`(state, msg) → (state, effects)` so that tests can drive it as a
value.

**Leave the four update layers in place and only fix projection
re-fetching.** Rejected as insufficient. Landing `H-TUI-001` alone
would improve responsiveness but would not remove the terminal-in-
handler pattern that blocks reducer-level interaction tests, would
not fix the modality bookkeeping cost, and would not stop each new
feature from adding its own effectful `Action` variant. The four
contracts land together as a coherent target; individual stories
land incrementally against that target.

**Rewrite the TUI in a fresh crate.** Rejected. The audit found the
foundations — single state, message enum, reducer, immediate mode,
async worker, snapshot tests — are the right ones. The remaining
work is convergence, not replacement.

## Open Questions Answered

- **Which architecture?** Elm/MVU with effects-as-data, modal stack,
  derived view-models. Contracts 1–5 above.
- **Framework?** No. Explicit non-goal.
- **Does the reducer become async?** No. `update` stays
  `fn(&mut App, Msg) -> Vec<Effect>`.
- **Does `RunConfig` remain live state?** No. Initial values only;
  UI state moves to `App`.
- **Is the viewer's private reducer part of the model or an
  exception?** Part of the model — it is the reference example for
  nested reducers via `Msg::Viewer(ViewerMsg)`, and its "eighth
  modal slot" status ends when it becomes a `Modal` stack entry.

## Open Questions Deferred

- **Concrete `Effect` catalog boundary.** `H-TUI-002` will land the
  first `Effect` enum from the audit-listed variants
  (`SpawnRefresh`, `RunMux`, `Exec`, `WriteStore`, `CapturePreview`,
  `Toast`, `Persist`, `Quit`). Whether preview capture stays a
  reducer-emitted effect or moves fully into the discovery worker's
  responsibility (post-refresh side-band) is a design question that
  reopens when `H-TUI-002` reaches that variant.
- **Persistence effect grain.** Per-view state persistence
  (`Effect::Persist`) may batch or debounce inside the executor;
  the shape of that batching is deferred until the store's write
  cost warrants it.
- **Nested-reducer composition style.** Whether nested reducers
  return `Vec<Effect>` directly or a `SubOutcome { msgs, effects }`
  that the parent lifts is a small ergonomic question deferred to
  the first non-viewer nested reducer.

## Related ADRs

- ADR 0031 (per-view state + filter / view switching) — the state
  the reducer owns per contract 4; this ADR moves it off
  `RunConfig`.
- ADR 0035 (SQLite-arc / index approaches) — Stage 1 status
  amendment tracks the substrate that `H-HYG-006` (`SnapshotIndex`)
  re-lands for contract 4.
- ADR 0057 (pin launch and terminal handoff) — the largest current
  effectful `Action` set; contract 2 gives it a home in the effect
  executor.
- ADR 0058 (resume splicing) — same shape as ADR 0057 for the
  effect executor's responsibilities.
- ADR 0067 (TUI snapshot mode) — the regression net contract 5's
  draw-purity rule preserves.
- ADR 0068 (snapshot fixture mode) and ADR 0069 (interactive
  fixture mode) — the non-live loop modes that share the event
  loop per contract 5.
- ADR 0078 (TUI surface division of labor) — the chrome-surface
  rubric that lives alongside this architecture rubric; both are
  guardrails for future TUI work.
