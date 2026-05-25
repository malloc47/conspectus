# ADR 0036: Embedded Query Engine Selection

## Status

Accepted

## Context

ADR 0035 established a staged approach to graph-to-view slicing: an
in-Rust `SnapshotIndex` selector layer (stage 1, landed) keeps the
view-builder code legible without committing to an external query
engine; later stages would escalate to Datalog (Ascent) or a SQL
engine (DuckDB/CozoDB) when triggered by concrete needs.

The stage-3 trigger has now fired: a user-facing
`conspectus query <expr>` surface is on the roadmap, with the working
plan recorded in `plans/stage-3-sqlite-query-engine.md`. ADR 0035 named
DuckDB and CozoDB as the leading stage-3 candidates and described the
decision as worth its own ADR with a spike. This ADR records that
decision.

The candidates revisited under the stage-3 head-to-head:

- **DuckDB.** Embedded analytic SQL via `duckdb` crate. Production-
  grade engine, recursive CTEs competitive on graph workloads
  (`USING KEY`, SIGMOD 2025), Parquet/CSV ingest, optional VSS for
  vector search, optional DuckPGQ for SQL/PGQ pattern queries.
  ~50–55MB bundled native lib. Single-writer file lock under the
  default storage mode.
- **SQLite.** Embedded transactional SQL via `rusqlite`. The most
  widely deployed embedded database in the world. WAL mode supports
  concurrent readers plus a single writer across processes on a
  shared host. Recursive CTEs without `USING KEY` (slower per-row on
  very large graphs, irrelevant at our scale). ~1MB bundled native
  lib. Vector search via the recent `sqlite-vec` extension.
- **CozoDB.** Embedded relational-graph DB with CozoScript Datalog.
  Graph-native query language; less mainstream than SQL. Active but
  smaller community than the other two.
- **In-Rust DSL on top of `SnapshotIndex`.** No external dependency;
  every saved view is a Rust function. Lower ceiling on user-facing
  ad-hoc query — the user has to commit to a Rust-call interface or
  we have to write our own parser.

The two architectural pressures that drove the decision:

1. **Workload shape.** The conspectus graph is OLTP-shaped: small
   (hundreds to low thousands of nodes), mostly-read,
   join/filter/group-by queries with a recursive minority (fork
   ancestry, session parent chains). It is not OLAP-shaped — no
   million-row aggregations, no columnar scans, no Parquet ingest
   on the roadmap. DuckDB's columnar machinery is optimising for a
   workload we do not have.

2. **Concurrency model.** The Phase 7 design assumes a continuous
   server *and* one-shot CLI invocations can read graph state
   simultaneously (`docs/design.md` §"Continuous Operation Mode"
   guarantees "absence of a server is not an error"). DuckDB's
   single-writer file lock fights this guarantee — workarounds
   require either a JSON-shared-canonical layer with DuckDB as a
   per-process cache (B3 in the prior plan) or an IPC-only model
   that breaks the no-server guarantee. SQLite's WAL mode is
   *designed* for exactly the multi-process reader plus single
   writer pattern.

## Decision

Adopt **SQLite** as the embedded query engine for the
`conspectus query <sql>` surface, with the following concrete
parameters.

### Crate and version

- Crate: `rusqlite >= 0.39` with the **`bundled`** feature mandatory.
- Bundled SQLite version floor: **3.51.3**. This release contains the
  fix for the WAL-reset corruption bug that affected SQLite 3.7.0
  through 3.51.2 in multi-process write/checkpoint contention — a
  bug class that maps directly onto our `conspectus serve` plus
  one-shot CLI pattern. A CI assertion fails the build if the
  bundled libsqlite3 version regresses below 3.51.3.
- License: SQLite is public domain; `rusqlite` is MIT/Apache. Both
  compatible with the Conspectus project license.
- MSRV: rusqlite's "latest stable at time of release" policy is
  more permissive than the candidate engines we ruled out. Pin the
  MSRV in `rust-toolchain.toml` alongside the rest of the workspace
  per ADR 0016.

### Extensions in scope at v1

- **JSON1** (always available with `rusqlite`'s bundled SQLite).
  Used for polymorphic blob columns (`SourceMetadata.fields`,
  `UnresolvedEndpoint.metadata`).
- **FTS5** (optional, kept available for future full-text search
  over preview text and PR bodies). Not required at v1.
- **`sqlite-vec`** is explicitly **deferred** to its own future ADR.
  P9-008 in `docs/backlog.md` stays blocked until that ADR exists.

### Boundaries

- The Rust resolver (`src/resolve/`) remains the source of truth for
  provenance precedence, relation-specific tie-breakers, and
  conflict diagnostics. SQLite is a *consumer* of resolver output,
  not a replacement for it. ADR 0041 records this as a non-decision
  so it does not get re-raised.
- The library API gates SQLite behind a `query` Cargo feature so
  consumers of `conspectus::api` who do not want the engine can
  build without it. ADR 0039 records the feature-gating model and
  amends ADR 0015.
- The bundled native lib's ~1MB binary delta is small enough that
  the existing distribution policy (ADR 0016) needs only a
  paragraph-sized amendment. ADR 0040 captures it.
- Snapshot persistence under SQLite is settled in P7-001's ADR
  deliverable, which absorbs the SQLite-specific format choices.
- Continuous-server transport under SQLite is settled in P7-004's
  ADR deliverable, which collapses around WAL-mode read sharing
  plus a Unix-socket write path.

### Process gate

No P9-* implementation story (`docs/backlog.md` §"Phase 9: Embedded
Query Engine") may land before this ADR and its dependent ADRs are
accepted. The dependency chain is:

```
0036 (this ADR) ─┬─→ 0037 (P7-001, persistence) ─→ 0038 (P7-004, transport)
                 ├─→ 0039 (library API)
                 ├─→ 0040 (distribution)
                 └─→ 0041 (resolver stays in Rust)
```

ADRs 0039, 0040, 0041 are independent of 0037/0038 and may land in
parallel with this one.

## Consequences

- A new optional native dependency enters the workspace. The
  one-binary release configuration bundles SQLite; library consumers
  opt in via `--features query`.
- The on-disk graph artifact changes from a versioned JSON snapshot
  to a SQLite database file (`graph.sqlite`) plus its WAL sidecars.
  JSON survives as a peer export format
  (`conspectus dump --format json`) for hand-inspection and
  portability. Existing JSON-snapshot expectations in `docs/design.md`
  §"Graph Snapshot Persistence" need rewriting once ADR 0037 lands.
- `conspectus query <sql>` becomes a real CLI surface. Saved views
  (P9-006) name the joins that today's hand-rolled view builders
  encode in Rust; ad-hoc SQL becomes the natural exploration tool,
  with the existing structured CLI commands (`session`, `table`,
  `node show`, TUI) continuing to use the in-Rust pipeline.
- The Phase 7 ADR dependency graph reshapes: P7-001 and P7-004 are
  no longer parallel because the storage choice that 0036 makes
  flows into the persistence ADR (P7-001) which in turn shapes the
  transport ADR (P7-004).
- The `H-OBS-*` hardening cluster (text-tree projection, filter
  flags, `--explain`) softens — once ad-hoc SQL exists, the natural
  exploratory and filtering tool is `conspectus query`. Each
  H-OBS-* story stays in the backlog but reassesses its priority
  after P9-004 ships.
- DuckDB-specific futures (Parquet ingest, columnar analytics over
  the snapshot, SQL/PGQ pattern syntax via DuckPGQ) are off the
  table at this engine choice. If any of them ever becomes a
  concrete roadmap deliverable, this ADR is revisited.

## Alternatives Considered

- **DuckDB.** The leading alternative under the prior stage-3 plan
  draft. Rejected for the conspectus workload because (a) the
  graph is OLTP-shaped, not OLAP-shaped — DuckDB's columnar
  vectorised engine is optimising for queries we do not run; (b)
  the single-writer file lock model fights the design.md §606–647
  one-shot-plus-server coexistence guarantee, forcing either a
  JSON-shared canonical layer with DuckDB as a per-process cache
  (a "two canonical stores" anti-pattern) or an IPC-only access
  pattern that breaks the no-server-needed promise; (c) the
  ~50–55MB bundled native lib is a substantial distribution-policy
  hit that ADR 0016 would need to absorb in a much larger
  amendment than SQLite requires. DuckDB remains the right
  candidate if graph sizes ever cross hundreds-of-thousands of
  nodes, or Parquet/CSV ingest becomes a roadmap need, or DuckPGQ
  matures and SQL/PGQ pattern syntax becomes a differentiator —
  none of which are on the current roadmap.
- **CozoDB.** Native graph-pattern Datalog (CozoScript). Rejected
  because Datalog is unfamiliar to most users compared to SQL, the
  community is smaller, and the data model would partially live in
  Cozo's relations rather than purely in the Rust source of truth.
  The user-facing query surface benefits more from SQL ubiquity
  than from Datalog's pattern syntax at our graph scale.
- **In-Rust DSL on `SnapshotIndex`.** Rejected because the user
  motivation for stage 3 (per ADR 0035 Q1 resolution) is both
  developer clarity *and* a user-facing query surface. A Rust-only
  DSL serves the first but not the second; we would still need a
  parser and runtime to land `conspectus query`, recreating most
  of what an embedded SQL engine ships for free.
- **Grafeo, IndraDB.** Too new (Grafeo, single-vendor) or
  server-oriented (IndraDB); neither fits the embedded one-binary
  shape.
- **Status quo (no engine, defer stage 3 again).** Rejected because
  the user has activated the stage-3 gate per ADR 0035 and
  recorded the requirement. Continuing to defer accumulates a
  hardening backlog (`H-OBS-001`, `H-OBS-003`, `H-OBS-004`) that
  is partially redundant with a real query surface.

## Open Questions Answered

- **Why now?** The Phase 7 persistence and server-transport ADRs
  (P7-001, P7-004) are both unstarted. Settling them before the
  engine choice would lock in a JSON-canonical format that fights
  a future SQL surface. Settling the engine now lets Phase 7's
  ADRs absorb the right concrete shape.
- **Why not defer one more round?** Every Phase 7 story not yet
  written depends on the engine question. Deferring keeps Phase 7
  blocked indefinitely. The cost of choosing wrong is paid by a
  ~1MB native lib and a feature-gated build; the cost of deferring
  is the rest of the workstream.
- **Why is SQLite's lack of `USING KEY` not a blocker?** Recursive
  CTEs without `USING KEY` are O(n²) in the worst case on
  unbounded transitive closures. The fork-ancestry, session-parent,
  and workspace-containment chains we recurse over are bounded by
  the depth of forks (rarely > 3) and the depth of session
  lineage (rarely > 5). At the graph sizes Conspectus targets,
  the per-row cost difference is dominated by the I/O of
  rendering, not the recursive expansion.
- **Does this constrain a future move to DuckDB or CozoDB?** No.
  The schema designed in P9-002 mirrors the in-Rust model, which
  is the source of truth. Migrating the storage layer is a
  loader-rewrite, not a model rewrite. The `SnapshotIndex`
  selectors stay unchanged.
- **What about WAL across a NFS/SMB mount?** SQLite's WAL mode
  requires shared memory between processes, which means all
  processes must be on the same host. Conspectus's continuous
  server and one-shot CLI are designed for a single-user,
  single-machine model (`docs/design.md` line 643). Network mounts
  remain unsupported, consistent with the existing scope.
