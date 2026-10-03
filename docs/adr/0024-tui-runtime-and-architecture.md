# ADR 0024: TUI Runtime, App Architecture, And Dependency Policy

## Status

Accepted

## Context

Phase 8 introduces `conspectus tui`, an interactive terminal UI over the
same resolved graph that backs the existing `graph`, `table`, and
`node show` commands. The product surface is locked
(`docs/implementation/phase-08-interactive-tui.md`,
`docs/tui-sessions-mockup.md`): a two-panel layout with a hierarchical
left-side row tree, a fixed header + preview right panel, polling
discovery, polling mux capture, async PR enrichment, single-key
attach, fuzzy search, multiple row-tree views (`sessions`, `mux`,
`union`, `prs`, `forks`), empty/loading/error frames, and a
responsive layout that re-flows for narrow and wide terminals.

The phase-08 plan calls out that the first implementation task must
produce an ADR before adding a TUI runtime dependency. Project
guardrails (`CLAUDE.md`) also forbid taking on new dependencies
casually: any new crate should be motivated, alternatives compared,
and the decision recorded as an ADR.

Three things need to be settled:

1. **Runtime library** — what Rust TUI crate to depend on.
2. **App architecture** — how to structure state, input handling,
   rendering, and async work inside the crate so the v1 surface can
   be built and the future surface (Phase 7 server transport,
   mouse, more views, more actions) can be added without rewrites.
3. **Dependency policy** — which additional crates the TUI work is
   permitted to pull in, and which require a follow-on ADR.

The implementation must respect the existing project posture: data-
model-first, provider-neutral core, library-first crate with a thin
CLI, deterministic outputs, and snapshot-friendly rendering.

## Decision

### 1. Runtime library: Ratatui + crossterm

Adopt **Ratatui** (current `0.30.x`) with the **crossterm** backend
as the v1 TUI runtime. No higher-level framework on top.

Rationale:

- Ratatui is the standard immediate-mode TUI library in the modern
  Rust ecosystem with active maintenance, broad ecosystem
  (`tui-tree-widget`, `tui-input`, etc.), and well-documented
  buffer/widget primitives.
- Crossterm provides the terminal lifecycle (raw mode, alternate
  screen, event polling) cross-platform without pulling in
  curses-style dependencies, which matters for the single-binary
  distribution story (ADR 0016) and the Nix dev shell.
- Immediate-mode rendering pairs well with the locked product
  surface: every refresh renders the full tree from a freshly-
  resolved graph snapshot, so we never have to mutate retained
  widget state from background tasks.
- Ratatui's buffer API is snapshot-test friendly. The phase plan
  calls for fixed-dimension buffer snapshots; Ratatui exposes a
  `Buffer` that the test harness can render into without a real
  terminal.

### 2. App architecture: in-tree Elm-style loop

Inside the crate, the TUI is structured as a deterministic Elm-style
app: pure state → view, with messages produced by terminal events,
timer ticks, and background tasks. No retained-mode framework
(tui-realm, Cursive) wraps Ratatui.

Module layout (extends `docs/implementation/phase-08-interactive-tui.md`):

- `src/tui/mod.rs` — public entrypoints invoked from `cli.rs`.
- `src/tui/app.rs` — `App` struct (the entire UI state),
  `Msg` enum (every event the loop processes), `update(App, Msg)
  -> (App, Vec<Cmd>)` reducer.
- `src/tui/cmd.rs` — `Cmd` enum describing side effects the
  reducer requests (refresh discovery, run gh, capture mux pane,
  attach to tmux, etc.). A small dispatcher executes commands and
  feeds results back as `Msg`s.
- `src/tui/data.rs` — graph-loading/refresh adapter. Owns the
  background discovery task and the per-provider freshness state.
  Shaped so a Phase 7 server snapshot transport can replace it
  without changing `app.rs`.
- `src/tui/rows.rs` — pure row-tree view-model builders for every
  table row-type (`CSP-163`). No Ratatui types here; outputs are
  plain Rust structs the renderer walks.
- `src/tui/detail.rs` — selected-node detail view-model
  (`CSP-164`).
- `src/tui/preview.rs` — preview adapters: mux pane capture
  (`CSP-168`) and un-muxed agent transcript-tail read (`CSP-171.03`).
- `src/tui/actions.rs` — attach (`CSP-169`) and resume
  (`CSP-170`) command construction. Pure planning, with exec
  handled by the command dispatcher.
- `src/tui/ui.rs` — Ratatui widgets and rendering. Consumes the
  pure view-models from `rows.rs` / `detail.rs` / `preview.rs`
  and the `App` for selection/focus state.
- `src/tui/runtime.rs` — terminal lifecycle: enter/leave alt
  screen, raw mode, panic hook for cleanup, the main event loop
  that polls crossterm events and timer ticks, dispatches
  `Cmd`s, and feeds resulting `Msg`s into `update`.

Boundaries that this layout enforces:

- **`update` is pure and synchronous.** Background work happens via
  `Cmd` execution, never inside the reducer. This keeps state
  transitions snapshot-testable without a runtime.
- **View models contain no `ratatui::*` types.** Row trees and
  detail view-models are plain structs the CLI can also consume.
  Snapshot tests for `rows.rs` / `detail.rs` test the data, not
  rendered terminal output.
- **`ui.rs` does not query global state.** It takes
  `&App` + view-models as input and draws to a `Frame`. Buffer
  snapshot tests render `ui::draw(&app, view_models, &mut frame)`
  into a fixed-size `Buffer` without a real terminal.
- **`runtime.rs` is the only module that touches the terminal.**
  Tests stub it out by driving `update` with synthetic `Msg`s.

### 3. Concurrency model

The reducer is single-threaded. Background work uses
**`std::thread::spawn`** with an `mpsc` channel back to the event
loop in v1. No `tokio`, no `async` runtime.

Rationale:

- The work items are coarse-grained — one discovery run, one `gh`
  call per selected PR, one tmux capture per cadence tick. They
  do not benefit from a task scheduler.
- Avoiding `tokio` keeps the binary smaller and the dependency
  graph closer to the rest of the crate, which is fully
  synchronous today (`gh`, git, harness adapters all shell out via
  `std::process::Command`).
- If a future capability (Phase 7 push transport, streaming
  transcripts) needs a runtime, the `Cmd` boundary lets us swap
  the dispatcher without churning the reducer.

Cancellation in v1 is best-effort: a stale background result for
a deselected row is dropped at receive time rather than aborted
mid-flight. This is acceptable for the v1 workloads — none are
long-running enough to matter.

### 4. Dependency policy

Add to `Cargo.toml`:

```toml
ratatui = { version = "0.30", default-features = false, features = ["crossterm"] }
crossterm = "0.28"
```

Allowed without an additional ADR, scoped to the TUI module:

- `tui-input` if and only if the v1 `/` search overlay needs more
  than a manual `String` + cursor index. Prefer hand-rolled text
  input first; add only if hand-rolled becomes fragile.
- `tui-tree-widget` — explicitly **not** adopted in v1. The row
  tree is built and rendered as a flat list with depth metadata
  so that scrolling, snapshotting, and ambiguous-mux expansion
  remain under direct control. Revisit if the tree gains
  drag-targets or other interactions a generic widget would
  better serve.

Requires a follow-on ADR before adoption:

- Any async runtime (`tokio`, `async-std`).
- Any heavyweight matcher (`nucleo`, `skim`, `fuzzy-matcher` ≥
  1MB compiled). v1 uses an in-tree case-insensitive substring +
  subsequence matcher; phase-08 explicitly defers fuzzy matching
  to a later ADR.
- Any retained-mode TUI framework on top of Ratatui (`tui-realm`,
  Cursive).
- Replacing crossterm with `termion` or `termwiz`.

This policy is narrower than the project-wide guardrail because
TUI dependencies tend to have outsized impact on binary size,
build time, and supported platforms. Conspectus distributes as a
single binary (ADR 0016); each new TUI dep needs justification.

### 5. Tooling and test posture

- Buffer snapshot tests use `insta` (already a dev-dependency) to
  serialize Ratatui `Buffer` contents at fixed terminal sizes. The
  exact `Buffer`-to-string helper lives in `src/tui/ui.rs` so the
  representation is stable across snapshot updates.
- The reducer is tested with `rstest` parameter sets to cover every
  `Msg` variant.
- Terminal lifecycle is exercised by a smoke test that constructs
  the runtime, immediately injects a `Msg::Quit`, and verifies
  graceful shutdown without touching a real TTY (the runtime
  exposes a `run_with_events(events: impl IntoIterator<Item =
  Msg>)` seam for this).
- A panic hook installed by `runtime.rs` restores raw mode and
  leaves the alt screen before unwinding, so `q`, Ctrl-C, and
  panics all leave the terminal usable.

## Consequences

- Phase 8 has a concrete runtime baseline. `CSP-162` can add the two
  dependencies and the terminal lifecycle without further design.
- The Elm-style boundary makes the reducer and view-models pure
  and snapshot-testable. The phase-08 snapshot test plan
  (`CSP-163`/`CSP-164`/`CSP-166`) lands without a runtime in the test
  harness.
- The dependency policy keeps the binary small. Two new crates
  (`ratatui` + `crossterm`) total roughly hundreds of KB of
  compiled code; no async runtime is pulled in.
- Future scope (Phase 7 server transport, push refresh, streaming
  transcripts) has a clear seam: `data.rs` and the `Cmd`
  dispatcher are the only modules that change.
- The CLI surface (`conspectus table`, `conspectus node show`) can
  reuse the same pure view-models the TUI consumes. This is the
  explicit hook for the table-parity work tracked alongside the
  TUI stories.

## Alternatives Considered

- **tui-realm on top of Ratatui.** Adds a component/property/event
  framework over Ratatui. Rejected for v1 because the locked
  product surface is small and bespoke (two panels, one tree, one
  status bar, a handful of overlays); the framework's mount/focus
  model would add layers without removing complexity. Reconsider
  if v2 grows many modal workflows.
- **Cursive.** Mature, view-tree + callback model with a broad
  widget library. Rejected because the row tree needs custom
  drawing for the ambiguous-mux expansion, color-aware mux glyphs,
  and responsive inline previews; Cursive's view abstractions
  fight that posture. Also less aligned with snapshot-test-first
  development than Ratatui's buffer API.
- **Raw crossterm / termion / termwiz.** Rejected because hand-
  writing tables, trees, scroll regions, selection state, and
  overlays from terminal primitives is avoidable risk for a v1
  with this scope.
- **Adopting `tokio` from day one.** Rejected because v1's
  background work (discovery + a few `gh`/tmux shells per second
  at most) does not need a task scheduler, and `tokio` would
  meaningfully expand binary size and dep graph for negligible
  benefit. A later ADR can adopt it if streaming or push transport
  changes the picture.
- **Pulling in `tui-tree-widget` for the row tree.** Rejected for
  v1 because the tree needs custom row composition (inline preview
  layering, mux glyph coloring, ambiguous-mux expansion). A flat
  list with depth metadata gives us full control and keeps
  snapshot tests deterministic. Revisit if interactions grow.
- **Pulling in a fuzzy matcher (`nucleo`, `skim`) up front.**
  Deferred per phase-08; the in-tree matcher is enough for v1,
  and a later ADR can adopt a library if quality demands it.
- **Retained-mode rendering inside Ratatui** (caching view trees
  across frames). Rejected because the graph is small, the
  display surface is small (80x24 to a few hundred cells wider),
  and immediate-mode rendering keeps snapshot tests trivial.

## Open Questions Answered

- The TUI runtime is Ratatui + crossterm, not tui-realm and not
  Cursive.
- The app architecture is an in-tree Elm-style loop with a pure
  reducer, command dispatcher, and immediate-mode render.
- v1 has no async runtime; background work runs on
  `std::thread::spawn` with `mpsc` channels.
- The new dependencies in this ADR are the only ones that land
  with `CSP-162`; any further TUI dep listed in the policy section
  requires its own ADR before adoption.
- View-models are pure and shared between TUI and CLI surfaces;
  the table command can adopt them as the row-tree builders land.
