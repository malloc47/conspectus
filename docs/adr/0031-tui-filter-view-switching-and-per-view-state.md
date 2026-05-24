# ADR 0031: TUI Filter, View Switching, And Per-View State

## Status

Accepted

## Context

The Phase 8 TUI ships today with a single sessions view, a single
`--sessions-grouping` knob, no filtering, and no way to move between
the other four locked views (`mux`, `union`, `prs`, `forks`) at
runtime. Phase 8 already locks `1`–`5` view-switch keys, a deferred
`/` search overlay (`T8-017`), and reserves a number of accelerator
keys (`m`, `c`, `f`, `n`, `d`, …) for later phases.

Three intertwined capabilities now need to land together rather than
piecemeal because they share keybindings, status-bar real estate,
config-file shape, and the CLI flag namespace:

1. **Filtering** — narrow the visible row set by structured
   predicates (operator example: "claude-only, nothing older than
   seven days").
2. **View switching** — move quickly between the five locked views
   without losing each view's place.
3. **Per-view grouping** — each view carries its own grouping enum;
   `--sessions-grouping` is the first instance of a pattern that
   needs to generalize.

Two design forks are large enough to warrant an ADR rather than
embedding them in feature stories:

- The relationship between **structured filters** and the deferred
  `/` **fuzzy search** overlay. Both narrow the visible set, but
  along different axes (structure vs ranking) and with different
  lifetimes (persistent vs transient).
- Whether **view state** (filters, grouping, expanded set,
  selection) is **global** or **scoped per view**. The choice
  cascades into the App state shape, the config schema, the CLI
  flag semantics, and the help / status-bar surfaces.

A third question — how operators discover these controls — is also
load-bearing because Phase 8 already exposes a dense single-key
keymap and the operator has flagged a strong preference for
menu-first discoverability with accelerators layered on top.

## Decision

This ADR settles five linked decisions.

### 1. Layered filter + search model

Structured filters and `/` fuzzy search compose; neither replaces
the other.

- **Structured filters** are typed predicates with explicit
  dimensions and values (e.g., `harness=claude`, `max-age=7d`,
  `mux-state=unmuxed`). They persist across navigation, render as
  status-bar chips, and survive view-state save/restore. They are
  the answer to "what world am I looking at."
- **`/` fuzzy search** (`T8-017`, unchanged scope) is a transient
  ranking overlay. It does not narrow the set persistently; it
  ranks the rows currently visible after filters apply.

The two layers compose in a fixed order: snapshot → filters →
fuzzy-search ranking → render. `T8-017` is updated only to note
that it ranks within the filtered set, not the full snapshot.

### 2. v1 filter dimensions

The v1 structured-filter set is locked at three dimensions:

- `harness` — set membership over the known harness keys
  (`claude`, `codex`, `opencode`, `aider`).
- `max-age` — duration window applied against
  `AgentSessionNode.last_active_epoch` (`H-AGENT-EPOCH`).
- `mux-state` — set membership over `attached` / `ambiguous` /
  `unmuxed`, derived from the row's `MuxIndicator`.

Free-text "contains" filtering is intentionally **excluded** from
v1; the `/` search overlay covers that need. Future dimensions
(branch, PR state, fork lineage, workspace, has-PR, has-fork,
declared-link state) are deferred to follow-on stories and may land
without amending this ADR provided they fit the existing
`RowFilter` shape.

### 3. Per-view state scope

The following state is **per view**:

- active `RowFilter`
- active `Grouping` (per-view enum; see §4)
- expanded-row set
- selection
- left-panel scroll offset

The following state is **global** (single value across all views):

- sort order (`Hierarchy` vs `Recency`)
- view-independent run config (refresh intervals, color, scan
  roots, live-preview flag)

Switching `1 → 2 → 1` returns the sessions view to the exact
filters / grouping / selection / scroll it had before the switch.
Sort stays global because the recency-vs-hierarchy choice is
view-independent in operator practice and inflating it to per-view
state introduces friction for the common case.

### 4. Per-view grouping enums

Each view has its own grouping enum. Sessions keeps the existing
`SessionsGrouping`; new enums land alongside the P8-004 builders:

```rust
pub enum SessionsGrouping { Graph, Repo, Checkout, ScanRoot }
pub enum MuxGrouping       { Session, Workspace, Host }
pub enum UnionGrouping     { Kind, Workspace, Repo }
pub enum PrsGrouping       { Repo, State, Workspace }
pub enum ForksGrouping     { Provider, Workspace, Parent }
```

A `Grouping` dispatch type wraps the per-view variants so the App
state, config schema, and CLI flag can carry a single field that
narrows to the active view's enum at access time.

### 5. CLI / TUI parity via a shared predicate type

A single `RowFilter` predicate type lives in a non-TUI module and is
consumed by both `conspectus tui` and `conspectus table <ROWS>`. The
CLI flag surface is defined once as a `FilterArgs` struct mounted on
both `TuiArgs` and `TableArgs`. The same `--harness claude
--max-age 7d --mux-state unmuxed` invocation narrows the static
table output and the TUI start state identically.

Per-view grouping is exposed as a single `--grouping <VALUE>` flag
whose accepted values depend on `--view`. Legacy
`--sessions-grouping` stays as an alias that prints a one-line
deprecation warning to stderr at parse time.

### 6. Discovery: menu-first, keys as accelerators

Every capability in this featureset is reachable through a
navigable **Controls overlay** with labelled options, arrow-key
navigation, Enter to commit / drill in, Esc to back out, and
mouse-click support when the terminal supports it. Sections cover
view selection, per-view grouping, per-view filter editing, and
the global sort.

Single-key accelerators (`1`–`5`, `]`/`[`, `f`, `F`, `v`, and the
grouping-cycle key chosen in `F8-005`) exist for muscle-memory
operators but are never the only entry point. Accelerator keys are
surfaced inline in the controls overlay and in the `?` help
overlay so they remain discoverable.

This decision overrides the implicit precedent set by Phase 8's
dense direct-row-action keymap. The keymap stays; this ADR adds a
discoverable surface for the new controls because their cardinality
(five views × N groupings × M filter dimensions) exceeds what a
direct-key map should bear.

### Config schema

`[tui.views.<name>]` sub-tables carry the per-view defaults. The
existing `[tui].sessions_grouping` key becomes an alias that emits
a one-line deprecation warning when both old and new are present
and seeds the new key when only the old is set.

```toml
[tui]
default_view = "sessions"
scan_roots = ["~/code"]
default_sort = "hierarchy"

[tui.views.sessions]
grouping = "graph"

[[tui.views.sessions.filters]]
harness = ["claude"]
max_age = "7d"

[tui.views.mux]
grouping = "session"
```

Multiple `[[tui.views.<name>.filters]]` array entries OR their
predicates together (set union). Single-entry inline tables are the
common case.

### Module layout

- `src/filter.rs` (new, crate-public) — `RowFilter`,
  `HarnessFilter`, `MuxStateFilter`, `MuxStateKey`. Pure predicate
  evaluation against `AgentSessionNode` plus resolved mux state.
  Lives outside `src/tui/` because `src/output/table*.rs` also
  consumes it. Per ADR 0024 dependency policy, the predicate type
  introduces no new dependencies.
- `src/tui/widgets/controls.rs` (new) — Controls overlay widget,
  per-section sub-editors, multi-select list primitive. Reuses the
  ADR 0030 text-input primitive for the `max-age` editor.
- Per-view grouping enums live next to their row-tree builders in
  `src/tui/rows/<view>.rs`.

## Consequences

- The TUI gains a discoverable controls surface that scales as more
  per-view options accumulate; future settings (density modes,
  workspace visibility, etc.) gain a natural home without growing
  the keymap.
- `conspectus table <ROWS>` becomes a scripting-grade surface
  parallel to the TUI: any narrowing the operator can configure in
  the TUI is reproducible at the CLI.
- Per-view state retention introduces a `ViewStates` map on `App`
  and additional reducer paths for view switching. The map is
  small (five views) so cost is bounded; tests cover save/restore.
- Help and onboarding need to point at the controls overlay first,
  with accelerators second. `docs/implementation/phase-08-
  interactive-tui.md` gains a "Controls overlay" subsection.
- The existing `--sessions-grouping` flag and `[tui]
  .sessions_grouping` key carry a deprecation cycle. Both keep
  working through at least one release; removal requires its own
  ADR addendum or a coordinated note.
- `T8-017` is no longer blocked on the filter design question. It
  remains its own story, scoped to ranking within the active
  filter set.

## Alternatives Considered

- **Single `/` query bar with mini-DSL** (`/claude age<7d foo`).
  Rejected. Denser syntax, no chip surface, harder for new
  operators to discover. The layered model gives both audiences
  what they need at the cost of two overlays.
- **Structured filters only, drop `/` search.** Rejected.
  Defers `T8-017` indefinitely and removes ranking semantics that
  filters cannot replace. The two layers answer genuinely different
  questions.
- **Global filter / grouping state across all views.** Rejected.
  Filter dimensions don't align across views (a `harness` filter
  on the PR view is nonsensical), and an operator's narrowing on
  sessions should not bleed into mux. Global sort is the one piece
  that is genuinely view-independent and stays global.
- **TUI-only filter flags, no `table` parity.** Rejected. Project
  design rule already shares view-models between `table` and the
  TUI; treating filters as a TUI-only feature would split the
  contract.
- **Dense single-key map without a controls overlay.** Rejected
  per operator feedback. The cardinality of the new options
  exceeds what direct-row-action keys can carry discoverably.
- **Modal command palette** (Ctrl-K style fuzzy command surface).
  Rejected for v1. Phase 8 already locked the no-command-palette
  decision; introducing one as a side effect of this featureset
  would re-open that. A navigable section list achieves the
  discoverability win without the palette.
- **Defer grouping enums for non-session views.** Rejected.
  Locking the enums now alongside the P8-004 builders avoids
  retrofitting and lets the controls overlay render every view's
  grouping uniformly from day one.

## Open Questions Answered

- Structured filters and `/` fuzzy search coexist; filters narrow
  persistently, `/` ranks transiently within the filtered set.
- v1 filter dimensions are `harness`, `max-age`, `mux-state`.
- Per-view state covers filters, grouping, selection, expanded
  set, and left-scroll; sort stays global.
- Each view has its own grouping enum.
- A single `RowFilter` predicate type drives both CLI and TUI.
- The primary discoverable surface is a navigable controls
  overlay; single-key accelerators are layered on top.
- `[tui].sessions_grouping` enters a deprecation cycle in favor of
  `[tui.views.<name>].grouping`.

## Open Questions (deferred)

- The exact primary key for the controls overlay (`v` proposed,
  with `Space` or a function-key alternative) is settled in
  `F8-005`.
- The grouping-cycle accelerator key (`G` collides with the
  existing End binding) is settled in `F8-005`.
- Mouse support for the underlying row tree (outside the controls
  overlay) is deferred to a separate story; this ADR commits only
  to mouse support inside the controls overlay.
- Additional filter dimensions (workspace, repo, branch, PR state,
  fork lineage, has-PR, has-fork, declared-link state) are
  follow-ups under the existing `RowFilter` shape and do not
  amend this ADR.
