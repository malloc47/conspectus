# ADR 0035: Graph-to-View Query Layer

## Status

Accepted (amended 2026-07-01 per the `docs/adr-audit.md` corpus audit).
The Stage 1 `SnapshotIndex` was displaced by the ADR 0043 SQLite
consumer surface before it landed and was not restored when ADR 0082
retired that surface; it is re-tracked as `CSP-467` in
`docs/backlog.md`. The Stage 2 (Ascent) and Stage 3 (`conspectus
query`) escalation gates are superseded by ADR 0082, which settled
typed-snapshot consumption as the consumer surface.

## Context

`GraphSnapshot` is the canonical data structure: nodes, candidate links,
resolved relationships, diagnostics, and the alias overlay. Every view
conspectus renders — the TUI sessions tree, the planned per-view filters of
ADR 0031 (Mux/Union/Prs/Forks), the CLI `table` projections, `node show`, the
detail pane — is a slice of that snapshot with filtering, grouping, and
enrichment layered on.

As views have multiplied, the *graph-to-view layer* has begun to fragment.
Two near-identical helper structs already exist:

- `SessionsIndex` in `src/tui/rows/sessions.rs` builds typed node maps
  (`agent_sessions`, `mux_sessions`, `repos`, `worktrees`) plus
  `by_source_relation: BTreeMap<(NodeId, RelationKind), Vec<&GraphLink>>`
  filtered to active candidate links, and exposes traversal helpers like
  `workspace_for_repo`, `workspace_for_session`, `checkout_for_cwd`,
  `mux_candidates_for_session`.
- `SnapshotView` in `src/output/table.rs` builds the same
  `by_source_relation` shape, the same active-link filter, an overlapping
  set of typed node maps (different node kinds — `forge_prs`, `forks` —
  but the same construction loop), and parallel `preferred_link` /
  `candidates_for` accessors.

Both structs also carry a private `pick_preferred` helper that ranks active
links by provenance precedence then confidence then link id — verbatim copies
of each other. `src/output/node_show.rs` and `src/tui/detail.rs` open-code
their own walks over `snapshot.candidate_links` for similar needs without
benefit of any index at all.

This duplication is not painful in isolation. It will become painful as
ADR 0031's remaining views land (Mux, Union, Prs, Forks each want some
combination of join + filter + group-by over the same edges), and it is the
proximate reason it has become hard to reason about *which slice of the graph
each view actually takes*.

A wider research pass evaluated whether an external query library should own
this layer. The credible candidates are:

- **Ascent** — compile-time Datalog as Rust macros. Strong fit for recursive
  shapes (fork ancestry, session parent chains, transitive containment) but
  not earning its keep against today's mostly-flat joins, and contributes
  little to a future user-facing query surface.
- **CozoDB** — embedded Datalog graph database. Datalog-native graph
  ergonomics, but Datalog is unfamiliar to most users and the integration
  pushes part of the data model into Cozo's schema.
- **DuckDB** — embedded analytic SQL with mature Rust crate, recursive CTEs
  competitive for graph workloads (`USING KEY`, SIGMOD 2025), and SQL/PGQ
  available via the research-grade DuckPGQ extension. Strongest candidate
  for a future `conspectus query <sql>` surface, but a ~50MB native lib
  is a heavy hammer for today's view-builder pain.
- **petgraph** — graph storage and algorithms, not a query language;
  orthogonal to the question.
- **GraphQL** (juniper / async-graphql) — optimised for consumer-driven
  nested projections, not joins/group-by. Wrong fit when conspectus owns
  both ends.

The full evaluation is in `plans/i-have-an-idea-refactored-boole.md`.

## Decision

Adopt a staged path with one near-term commitment and two named escalation
gates.

**Stage 1 (now).** Introduce an in-Rust `SnapshotIndex` selector layer at
`src/model/index.rs`, factor the duplicated indexing out of `SessionsIndex`
and `SnapshotView`, and migrate the existing views to consume it. No new
dependency. The layer carries:

- Typed node maps for every `NodeKind` that any current view needs
  (`agent_sessions`, `mux_sessions`, `repos`, `checkouts`, `workspaces`,
  `branches`, `forks`, `forge_prs`).
- `by_source_relation: BTreeMap<(NodeId, RelationKind), Vec<&GraphLink>>`
  over active candidate links, the canonical join key both existing index
  structs already share.
- Named selectors for the joins both consumers were already coding by hand:
  `preferred_link`, `candidates_for`, `mux_candidates_for_session`,
  `checkout_for_path`, `checkout_count_for_repo`, `workspace_for_repo`,
  `workspace_for_session`, `repo_display_path`.
- A single canonical `pick_preferred` helper exported from the same module
  so per-view ranking does not drift.

The layer is read-only and constructed once per snapshot. View-specific
inverse indexes (e.g. `attached_to_mux`, computed only by the table
renderer) stay in the view that needs them, built on top of the shared
index rather than duplicating its construction loop.

**Stage 2 (gated on a measured trigger).** Re-evaluate Ascent when ≥ 2
in-flight views need recursive closures that are awkward as iterators
(fork ancestry, session parent chains, transitive workspace/repo
containment), or when a single view ships an imperative closure that the
review-reader cannot grasp in one sitting. No commitment to Ascent in
advance.

**Stage 3 (gated on a roadmap trigger).** When `conspectus query <expr>`
becomes a concrete roadmap deliverable, evaluate DuckDB and CozoDB
head-to-head in a dedicated ADR with a small spike. Likely outcome: DuckDB
wins on SQL familiarity and engine maturity unless DuckPGQ matures or
graph-pattern queries dominate the workload, in which case CozoDB's
native Datalog ergonomics flip the choice. Both engines can load from the
stage-1 selector module, so the choice can be deferred without sunk cost.

`SnapshotIndex` is the substrate for every later stage; it is additive,
not replaced.

## Consequences

- `src/tui/rows/sessions.rs` (`SessionsIndex`) and `src/output/table.rs`
  (`SnapshotView`) lose their bespoke index types and consume
  `SnapshotIndex` directly. View code is shorter and the join key is now
  named in one place.
- Future TUI views (Mux/Union/Prs/Forks per ADR 0031) inherit the
  selectors instead of re-implementing the loop. A new view becomes
  "pick a starting set, chain selectors, enrich, render."
- `node_show` and `detail.rs` continue to open-code their walks for now,
  but the shared module is the obvious next step when those become
  painful in their own right. Their migration is not blocking on this
  ADR.
- One new module under `model/`. No new dependency, no compile-time hit,
  no binary-size impact, no change to the public API surface beyond
  re-exporting `SnapshotIndex` from `crate::model`.
- The escalation gates are *named* and *measured*, not implicit. The next
  evaluation has a concrete trigger and will produce its own ADR rather
  than accumulating drift in the view layer.

## Alternatives Considered

- **Do nothing; let each new view grow its own ad-hoc index.** Rejected.
  The duplication already exists between two views; ADR 0031 multiplies
  it. The cost of factoring grows superlinearly with the number of
  consumers.
- **Adopt CozoDB now.** Rejected. Solves a pain we do not yet have (a
  user-facing query surface) while costing a heavyweight dependency and a
  partial relocation of the data model into Cozo's schema. The
  developer-clarity pain the current evaluation surfaced is in indexing
  and joining, which the typed selectors cure with no external cost.
- **Adopt DuckDB now.** Rejected for stage 1 for the same reason as
  CozoDB plus a steeper binary-size cost; but explicitly reserved as the
  leading candidate at stage 3 when general query becomes a concrete
  roadmap deliverable.
- **Adopt Ascent now.** Rejected. Ascent is best when recursion dominates,
  and today's view shapes are mostly 2–3 hop joins. Gated to stage 2 with
  a named trigger.
- **Add petgraph as a storage backend.** Rejected as orthogonal. Petgraph
  is a graph data structure plus algorithms; it does not provide
  SQL/Datalog/Cypher semantics and does not address the duplication.
  Could complement any later stage if traversal performance ever becomes a
  bottleneck, but offers no payoff at stage 1.
- **GraphQL (juniper / async-graphql) executed locally.** Rejected. The
  pain is joins and group-by; GraphQL is optimised for consumer-driven
  nested projections, which conspectus does not need when it owns both
  ends.

## Open Questions Answered

- **Where does `SnapshotIndex` live?** `src/model/index.rs`, re-exported
  from `crate::model`. The index is a derived view of the model, so it
  belongs alongside the model rather than under `tui/` or `output/`.
- **Does the model change?** No. `GraphSnapshot` and the node / link /
  relationship types are unchanged. The index is purely a read-only
  derived structure.
- **Are typed node maps allocated for views that do not need them?**
  Yes. The cost is one `BTreeMap` per node kind per snapshot build. This
  is the same shape that `SessionsIndex` and `SnapshotView` already
  build today; the difference is they now share the work.
- **Does this constrain the stage 2 / stage 3 choice?** No. Ascent
  consumes typed iterators that `SnapshotIndex` already exposes. DuckDB
  and CozoDB both expect a load step that materialises nodes and links
  into their tables/relations; the index's typed views are exactly the
  right input shape.
- **What about `node_show` and `detail.rs`?** Out of scope for the
  initial migration. They open-code their walks today without a
  duplicated index struct, so factoring them is lower priority and can
  follow once the second TUI view (per ADR 0031) lands and shakes out
  any missing selectors.
