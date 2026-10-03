# ADR 0042: Vector Search Via `sqlite-vec`

## Status

**Superseded** by [ADR 0082](0082-retire-sqlite-persistence-and-query-surface.md).

The vector-search surface this ADR defined attached to
`conspectus query --similar-to`, which is gone (CSP-447
removed the `query` subcommand). The `embeddings` overlay
table, the `--load-extension` flag, the import follow-up
(CSP-293), and `docs/vector-search.md` all retire alongside
the query feature. Reopening vector search would be a separate
ADR landing against a different surface (e.g. against the
JSON-dump export rather than an embedded SQL engine), if and
when demand reappears.

Original status: Accepted.

## Context

ADR 0036 chose SQLite as the embedded query engine for
`conspectus query <sql>` and explicitly **deferred** vector search to
its own ADR. The Stage 3 plan in `plans/stage-3-sqlite-query-engine.md`
named this as `ADR-G`, and the backlog story `CSP-278` was opened
gated on this ADR.

The motivation for vector search inside Conspectus is bounded but
real: agent sessions accumulate text artifacts (the
`last_message_preview` field, future transcript surfaces) that
operators sometimes want to find by similarity rather than by exact
match. A future `conspectus query --similar-to <session-id>` flow
gives the user "find me the other sessions that look like this one"
without leaving the SQL surface.

The question is the engine choice, the storage shape, the ingestion
lifecycle, and the v1 scope.

## Decision

Adopt **`sqlite-vec`** (Alex Garcia's vector-search SQLite extension)
as the engine. Land **v1 as schema + query plumbing** with a sensible
fallback when the extension is absent; defer the runtime extension
distribution and the embedding-computation pipeline to follow-up
stories.

### Engine: `sqlite-vec`

- Pure C, no dependencies, runs anywhere SQLite runs (Linux, macOS,
  Windows, WASM). Stable since v0.1.0 (July 2024).
- Recent independent benchmarks (gist1m dataset) show
  `sqlite-vec` outperforming DuckDB VSS on small-to-medium vector
  workloads by an order of magnitude on both build and query time
  — consistent with the OLTP-shaped argument that drove the ADR
  0036 SQLite choice.
- Provides `vec0` virtual tables for indexed KNN search plus a
  family of scalar functions (`vec_distance_cosine`, `vec_l2`, etc.)
  usable against any BLOB column without the virtual table.
- Distributed as a runtime-loadable SQLite extension. **No new
  Rust crate** is required; we load it via rusqlite's existing
  `Connection::load_extension`.

### Storage shape

Three additions to `schema.sql`:

```sql
-- One row per (node, embedded field, model) embedding. Polymorphic
-- over which field carries the embedded text; the loader decides.
CREATE TABLE IF NOT EXISTS embeddings (
    node_id      TEXT    NOT NULL,
    source_field TEXT    NOT NULL,                 -- e.g. 'last_message_preview'
    model        TEXT    NOT NULL,                 -- model identifier, opaque to conspectus
    dim          INTEGER NOT NULL,                 -- vector dimensionality
    vector       BLOB    NOT NULL,                 -- float32 little-endian, dim * 4 bytes
    PRIMARY KEY (node_id, source_field, model)
);

CREATE INDEX IF NOT EXISTS idx_embeddings_source_field
    ON embeddings(source_field, model);
```

The `vec0` virtual table is created **on demand by the runner** at
connection-open time when `sqlite-vec` is loaded:

```sql
CREATE VIRTUAL TABLE IF NOT EXISTS embeddings_vec USING vec0(
    embedding float[<dim>]
);
```

Conspectus does **not** ship the `vec0` table in `schema.sql` because
`CREATE VIRTUAL TABLE ... USING vec0(...)` errors when the extension
is not loaded. Keeping the runtime decision out of the canonical
schema keeps the schema applicable to every build of Conspectus
regardless of whether the user has built and loaded `sqlite-vec`.

The dimensionality of the `vec0` virtual table is a constraint of
the extension; v1 assumes **all embeddings in a database share a
single `dim`**. Mixed-dim corpora are out of scope until a follow-up
ADR addresses multi-model storage.

### Ingestion lifecycle

Conspectus **does not compute embeddings**. Computing them requires
an external embedding model whose choice (cost, latency, dimension,
licensing) is the user's call, not Conspectus's. v1 specifies an
import path; the actual embedding computation happens upstream.

Two ingestion paths are sanctioned:

1. **External tool, JSON Lines via stdin** — a separate command
   (`conspectus query --import-embeddings`, CSP-278-followup) reads
   `{"node_id": "...", "source_field": "...", "model": "...", "vector": [...]}`
   lines and inserts them. This is the documented path for users
   who run their own embedding pipeline.
2. **SQL `INSERT`** — operators with their own ingestion plumbing
   can `INSERT INTO embeddings(...)` directly through any writer
   connection. Mutating commands route through the server's writer
   per ADR 0038.

Ingestion is **decoupled from discovery**: discovery never produces
embeddings, the resolver never reads them, and the warm-start path
ignores them. They are an additive overlay maintained outside the
normal provider-eviction lifecycle.

### Query surface

`conspectus query --similar-to <node-id>` resolves to a built-in
KNN query against the `embeddings` table. Three flag dimensions:

- `--field <name>` (default `last_message_preview`) selects which
  embedded field to compare against.
- `--limit N` (default `10`) caps the result count.
- `--load-extension <path>` optionally loads `sqlite-vec` at
  connection-open time. When omitted, the runner does a
  Rust-side linear cosine-similarity scan over the `embeddings`
  table.

The runner picks its query path automatically:

- **Extension loaded**: `INSERT INTO embeddings_vec(embedding)
  SELECT vector FROM embeddings WHERE source_field = ?1`, then
  `SELECT ... FROM embeddings_vec ... LIMIT ?N`.
- **Extension absent**: read every matching `embeddings.vector`
  into Rust, compute cosine distance against the target vector,
  return top-N. Cost is O(n × dim); fine for graphs in the
  thousands-of-nodes regime conspectus targets.

The fallback keeps `--similar-to` working out of the box for users
who do not maintain a built `sqlite-vec` binary. Performance with
the extension is several orders of magnitude faster on large
corpora; for conspectus's scale the difference is "indistinguishable
from instant" either way.

### v1 scope (this ADR)

- `embeddings` regular table in `schema.sql`.
- Schema version bumped per ADR 0037's policy.
- Runner gains the `--similar-to / --field / --limit /
  --load-extension` flags.
- Runner contains the Rust linear-scan fallback.
- Runner attempts `sqlite-vec` extension loading when
  `--load-extension` is supplied; sets up `embeddings_vec` if
  loading succeeds.
- The loader's snapshot-import path does **not** touch embeddings.
  Embeddings persist across snapshot rebuilds because they live
  in a separate table that the loader does not clear.

### Out of scope (deferred to follow-up stories)

- `conspectus query --import-embeddings` ingestion command.
- Pre-built embedding model integration (Conspectus stays
  embedding-model-agnostic).
- Multi-model / multi-dim corpora in the same database.
- PR title / body embeddings (`ForgePrNode` does not carry text
  fields today; revisit when the model gains them).
- Bundling `sqlite-vec` into the Conspectus binary (current plan
  is runtime extension loading from a user-provided path; static
  bundling is a distribution-policy question that belongs in an
  ADR 0040 amendment).

## Consequences

- A new table (`embeddings`) joins the schema. Schema version
  bumps to 2.
- Three new CLI flags on `conspectus query` (`--similar-to`,
  `--field`, `--limit`, `--load-extension`).
- The `loadable_extension` rusqlite feature is added to `Cargo.toml`
  so `Connection::load_extension` is available; binary size delta
  is in the kilobytes.
- The `--similar-to` flag works correctly without `sqlite-vec`
  installed — the linear-scan fallback ensures the user can iterate
  on the embeddings table even before the extension is built.
- Embeddings persist across snapshot rebuilds because the loader
  never clears the `embeddings` table. Stale embeddings (vectors
  for nodes that no longer exist in the graph) survive until
  manually pruned; a `DELETE FROM embeddings WHERE node_id NOT IN
  (SELECT node_id FROM v_nodes)` is the documented garbage-
  collection idiom.
- The `CSP-100` cache layer item now has a sibling concern:
  embedding refresh. The two cache-layer hardening stories should
  be designed together once they reach the top of the backlog.

## Alternatives Considered

- **DuckDB VSS.** Rejected for the same reasons ADR 0036 rejected
  DuckDB as the engine: OLAP-shaped, fights the WAL-driven
  concurrency model, much larger binary. Recent benchmarks
  additionally show `sqlite-vec` outperforming VSS on the small-
  scale workloads conspectus runs.
- **`chromem-go` / `lancedb` / a separate vector store.** Rejected
  as a second canonical store on top of SQLite. The Stage 3 plan's
  whole point is that one canonical store is simpler than two; a
  separate vector DB would re-introduce the synchronization
  problem the SQLite pivot solved.
- **Pure Rust KNN, no SQL extension.** Rejected as the *exclusive*
  story but kept as the fallback. Pure-Rust KNN cannot benefit
  from `sqlite-vec`'s indexes, so it scales poorly past 10k
  vectors; for Conspectus's current scale it is fine, but giving
  up the indexed path is unnecessary when the upgrade is "load an
  extension on connection open."
- **Bundle `sqlite-vec` statically into Conspectus's binary.**
  Tempting because it would deliver indexed KNN out of the box.
  Rejected for v1 because static bundling requires either
  forking `libsqlite3-sys` or building a parallel libsqlite3-vec-
  sys crate, and the runtime-loadable path is enough for the
  initial workload. Revisit if vector search becomes the
  dominant query pattern.
- **Compute embeddings inside Conspectus.** Rejected. Embedding
  computation is a domain Conspectus has no business owning: the
  model, the cost, the GPU access, the rate-limits are all the
  user's call. Conspectus stores and queries; the user computes.

## Open Questions Answered

- **Why not ship the `vec0` virtual table in `schema.sql`?**
  Because `CREATE VIRTUAL TABLE USING vec0(...)` errors when
  `sqlite-vec` is not loaded. Keeping vec0 creation out of the
  canonical schema lets the schema apply uniformly regardless of
  whether the user has built the extension. The runner creates
  the virtual table on demand when the extension loads
  successfully.
- **What model do v1 embeddings target?** The model identifier is
  opaque to Conspectus and stored verbatim. v1 documentation will
  point at common 384-dim models (e.g. `all-MiniLM-L6-v2`) as a
  starting point, but the schema accommodates any dim a user
  picks. A multi-model corpus (mixed dims in the same database)
  is deferred to a follow-up ADR.
- **Does the loader touch embeddings?** No. The loader clears and
  rewrites node tables, candidate links, and resolved
  relationships, but the `embeddings` table is preserved across
  loads. Embeddings are computed externally and persist
  independently. Stale rows (embeddings for nodes that no longer
  exist) survive; users prune them manually with the documented
  `DELETE` idiom or via a future garbage-collection command.
- **What happens when the user supplies `--load-extension` with a
  bad path?** The runner returns a clear error from
  `Connection::load_extension` and does not silently fall back to
  the linear scan. The fallback only triggers when the user
  *omits* `--load-extension` (intentional opt-out).
- **Does this require the `loadable_extension` rusqlite feature?**
  Yes. We add it to `Cargo.toml` in CSP-278. The feature has no
  runtime cost when no extension is loaded; it just exposes the
  `load_extension` method on `Connection`.
- **What about ADR 0038's write-path routing?** Embeddings inserts
  are mutations that go through the same Unix-socket-to-server
  path under continuous mode, or take the writer lock directly
  when no server is running. The query path remains read-only.
