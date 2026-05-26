# ADR 0043: SQLite As The Sole Consumption Surface For Views And Renderers

## Status

Accepted

## Context

ADR 0037 made SQLite the canonical persisted graph artifact. ADR 0041
established that the resolver stays in Rust and writes its output into
SQLite. ADR 0035 introduced `SnapshotIndex` as the in-memory selector
layer that view-builders (TUI rows, CLI table projections, `node show`,
the detail pane) consume.

The result is a system with two parallel read paths over the same
data:

- **In-memory.** TUI and CLI renderers consume `GraphSnapshot` via
  `SnapshotIndex`. Lives in `src/tui/rows/`, `src/output/table.rs`,
  `src/output/node_show.rs`, `src/tui/detail.rs`.
- **SQL.** `conspectus query <sql>` and the curated saved-view library
  (`v_sessions_with_repo`, `v_mux_attachments`, `v_pr_by_branch`,
  `v_fork_ancestry`, `v_workspace_member_repos`) read the same tables.

Within a single process invocation both paths derive from the same
loader output and cannot drift, but they require maintaining two
join/selection vocabularies (`SnapshotIndex::preferred_link` etc. *and*
the saved-view DDL) for what is logically the same query. As the
backlog adds more views — Mux/Union/Prs/Forks per ADR 0031 — the
duplication grows.

A spike on `phase-9-sqlite-spike` validated both halves of a possible
consolidation:

- `src/query/reader.rs` reverses the loader and round-trips the full
  fixture corpus losslessly.
- `src/output/agent_sqlite.rs` reimplements 9 of the agent projection's
  17 cells reading from SQLite (`v_sessions_with_repo` + a small
  `LEFT JOIN`) and reuses the existing `RenderOptions` / column
  registry / `render_rows` substrate unchanged.

The spike showed the shape works, surfaced two schema concerns worth
addressing before a broad migration, and confirmed that filter /
grouping logic does not need to migrate into SQL.

## Decision

**SQLite is the sole consumption surface for view, render, and
inspection code in Conspectus.** `GraphSnapshot` is demoted to a
producer-side intermediate, scoped to the discovery → resolver →
loader pipeline. Consumers — the TUI row builders, every CLI table
projection, `node show`, and the detail pane — read from a
`rusqlite::Connection` and stop holding a `GraphSnapshot`.

This complements rather than contradicts ADR 0041. The resolver
continues to be implemented in Rust and writes typed
`ResolvedRelationship` rows into SQLite; consumers read those rows
back. "Resolver in Rust" is a producer-side implementation detail,
"SQLite is the read surface" is a consumer-side contract, and the two
positions stand independently.

### Concrete commitments

1. **`GraphSnapshot` is producer-only.** Once the loader has run on
   discovery+resolver output, the snapshot is dropped. No consumer
   holds a reference; no public API returns one for rendering
   purposes. The JSON export path (`conspectus dump --format json`)
   reads from SQLite and reconstructs the snapshot on demand (the
   spike's `read_snapshot()` is the entry point).

2. **`SnapshotIndex`, `SnapshotView`, and the `SessionsIndex`-style
   per-view indexes are retired** once their consumers migrate. The
   typed maps and `preferred_link` / `candidates_for` selectors have
   SQL analogues (`SELECT … WHERE source_node_id = ?` and joins
   against `resolved_relationships`); the migration replaces each
   call site.

3. **Filter and grouping logic stays in Rust.** `RowFilter`
   (ADR 0031), the TUI sessions bucketing, the launch-context
   highlight, and the recency-bucket styling sit on top of the SQL
   result set. They are not part of the SQL surface.

4. **A shared rendering substrate emerges in `src/output/`.** The
   `RenderOptions` struct, `ColumnSpec` registries, `render_rows`,
   `format_relative_age`, and the FNV-1a short-id hash are all
   backend-agnostic and move into shared modules that both in-memory
   and SQLite renderers consume during the staged migration. After
   migration the in-memory renderer is removed.

5. **Saved views become load-bearing.** `v_sessions_with_repo` and the
   other curated views shift from "documentation aids" to "the joins
   the renderers compile against." Bumping `SCHEMA_VERSION` is
   required for any shape change to these views.

### Schema work required before broad migration

The spike surfaced two issues that must be resolved first; both are
contained and well-scoped.

#### NodeId reconstruction in foreign-reference tables

`candidate_links.source_node_id`, `candidate_links.target_node_id`,
`resolved_relationships.{source,target}_node_id`,
`diagnostics.conflict_source_node_id`, and `aliases.node_id` store
the `fmt::Display` form of `NodeId` as `TEXT`. Reading them back
requires parsing, and the `Display` form is not robustly parseable:
common_dir / refname / path values may legitimately contain `@`,
`:`, `#`, or `/`.

The spike's `parse_node_id` works for every existing fixture and
will work in practice for typical filesystems, but the model permits
characters that break it. We commit to fixing this before the broad
migration, with two acceptable resolutions:

- **Structured-id columns (preferred).** Add `*_kind` discriminator
  columns plus per-kind structural columns alongside each foreign
  reference (e.g. `candidate_links` gains `source_kind`,
  `source_repo_common_dir`, `source_session_harness_key`, …). The
  loader already has typed `NodeId` at write time, so the cost is
  schema width and indexing. Eliminates parsing.
- **`impl FromStr for NodeId` with escaping (fallback).** Promote the
  spike's parser to a proper round-trip with escape sequences for
  separator characters. Cheaper schema-wise but breaks human
  readability of `node_id` text and adds an invariant to every
  `Display` consumer.

The structured-id approach is the recommendation; the FromStr
approach is recorded as the alternative if the schema widens
unworkably.

#### Compile-time exhaustiveness on read

Today the loader's `insert_*` functions destructure their typed
input exhaustively, so adding a field to a node struct breaks the
build until the loader updates. The spike's reader (`row.get(N)`
calls against ordered SQL columns) has no such enforcement; a field
addition can silently leave the reader producing a default.

Before migrating consumers, introduce a per-table read helper (a
trait, a macro, or hand-written paired-arrays of column lists +
typed-row mappers) that the compiler can use to flag a node-kind
schema drift between loader writes and reader reads. The
`saved_views_match_schema` test pattern (ADR 0036/0037 era) is the
precedent.

### Migration staging

The migration is broken into the backlog stories below (see Phase 10
in `docs/backlog.md`). Briefly:

1. Substrate: lossless round-trip test, structured-id refactor,
   reader exhaustiveness enforcement, shared rendering helpers
   extracted into a backend-agnostic module.
2. Per-renderer migration, one at a time, in increasing complexity:
   agent projection → mux/union/prs/forks projections → `node show`
   → TUI detail pane → TUI rows (sessions, then the ADR 0031 views).
3. Retirement: delete `SnapshotIndex`, `SnapshotView`,
   `SessionsIndex`. Remove the in-memory renderer dual-path code.
   Demote `GraphSnapshot` to producer-only and gate its public
   re-export accordingly.

Each renderer migration is its own backlog story so the work can
land incrementally on `main` behind unit-tested parity.

## Consequences

- One read vocabulary instead of two. New view-shapes are added once,
  as SQL (likely a new saved view); the renderer wires up to it
  through the typed-row layer. The pattern in `agent_sqlite.rs` is
  the template.
- The saved-view library becomes a load-bearing public-ish surface;
  changes to it require `SCHEMA_VERSION` bumps and renderer-side
  updates. `docs/query-guide.md` and the renderer code stay in
  lockstep.
- `GraphSnapshot`'s role narrows. It remains the resolver's input/
  output type and the JSON-export wire shape, but no renderer holds
  one across an entire invocation. The in-memory model is still
  useful for tests and snapshot-driven fixtures; nothing about the
  decision changes that.
- The compile-time safety the typed `SnapshotIndex` gave consumers
  (variant additions trip exhaustive matches) is lost on the SQL
  side. The reader-exhaustiveness work (above) and per-projection
  typed-row structs recover most of it. The remainder — "every
  `RelationKind` is handled by the right view" — is enforced by the
  existing `saved_views_match_schema` test and analogous tests added
  per renderer.
- The performance characteristic shifts from "one snapshot build +
  typed-map lookups" to "one prepared SQL statement per render." The
  spike's prototype is well within acceptable bounds for the CLI; the
  TUI refresh loop is the place to measure during its migration.
  WAL keeps reads concurrent with writers.
- ADR 0035 (`SnapshotIndex` substrate) is superseded for consumers
  once their migrations land. The selector layer remains valid
  producer-internal infrastructure and may persist there.

## Alternatives Considered

- **Keep the dual read path.** Rejected. The cost of maintaining two
  vocabularies grows with each new view; the in-process consistency
  guarantee is real but does not justify the duplication or the
  "which surface is canonical for this question?" friction.
- **Move the resolver into SQL** (the reverse of ADR 0041). Rejected;
  ADR 0041 already settled this and the spike confirms it is not
  needed to achieve a single consumption surface. SQLite-as-consumer
  works regardless of where the resolver lives.
- **Materialize the join the renderer needs into a fat `v_agents`
  view (and one per projection) and keep `GraphSnapshot` as the
  consumer-side type, populated by reading those views.** This
  consolidates the join vocabulary but keeps two consumption paths
  in the renderer code. Rejected for the same reason: the duplication
  surface stays.
- **Stop at the round-trip test; do not migrate renderers.** Rejected.
  The round-trip test is necessary but not sufficient — it proves
  the loader is lossless, which keeps the option open, but the
  duplicated indexing layer is what produces operational cost. The
  spike's `agent_sqlite.rs` shows the migration is mechanical.
- **Defer the schema fixes (NodeId parsing, reader exhaustiveness)
  and ship the migration in parallel with the workarounds.**
  Rejected. Both fixes are bounded and would otherwise be retrofitted
  per-renderer, with predictable drift.

## Open Questions Answered

- **Does this require `GraphSnapshot` to be removed from the public
  API?** No. `GraphSnapshot` remains the resolver's input/output
  contract and the JSON export wire shape. The change is that
  consumer code (renderers) no longer takes it as a parameter.
  Library callers who want a snapshot in memory still call
  `read_snapshot()` on a `Connection`.
- **What about `conspectus dump --format json`?** It calls
  `read_snapshot()` on the persisted database and serializes the
  result with `render_graph_json`. ADR 0037 already names this
  shape; the spike's reader is its implementation.
- **Does the TUI keep working without `SnapshotIndex`?** Yes, but
  every grouping/filtering/launch-context behavior gets re-anchored
  on typed-row results instead of typed-node maps. The spike doesn't
  prototype the TUI side because the agent projection is the more
  join-heavy surface; the TUI migration is a backlog story.
- **How is the migration tested for parity?** Each renderer story
  ships with snapshot tests that drive both the old and new
  renderers from the same fixture and assert identical output. The
  old renderer is removed only after parity tests pass across the
  fixture corpus.
- **Does this preclude future graph-query engines (CozoDB, etc.)?**
  No. ADR 0035's stage-3 escalation gate remains open. If a future
  ADR replaces SQLite with another embedded engine, this ADR's
  position translates ("the engine is the sole consumption
  surface"); only the implementation of the typed-row layer changes.
