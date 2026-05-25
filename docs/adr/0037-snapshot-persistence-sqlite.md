# ADR 0037: Snapshot Persistence Under SQLite

## Status

Accepted

## Context

`docs/backlog.md` Phase 7 (`P7-001`) called for an ADR settling the
on-disk snapshot format and lifecycle. The original scope assumed a
versioned JSON document under
`$XDG_DATA_HOME/conspectus/snapshots/`, atomically renamed, with
per-provider freshness encoded as a side-map.

ADR 0036 changed that. The embedded query engine selection adopts
SQLite as the user-facing query backend. With SQLite present, the
natural primary persisted store is the SQLite database itself —
JSON-as-canonical creates a "two canonical stores" anti-pattern in
which the SQL surface either pays a load cost per invocation or
ends up out of sync with the JSON it derives from.

This ADR settles `P7-001`'s deliverable in the SQLite-aware shape.
It is the persistence half of the Phase 7 ADR pair (the transport
half is ADR 0038).

The `docs/design.md` §"Graph Snapshot Persistence" (lines 649–692)
describes the JSON-primary design under the prior assumption. That
section needs rewriting after this ADR lands; the rewrite is
queued as part of the Phase 9 design-doc revisions named in
`plans/stage-3-sqlite-query-engine.md`.

## Decision

The canonical persisted graph artifact is a **SQLite database** at
`$XDG_DATA_HOME/conspectus/graph.sqlite`, in WAL mode. JSON survives
as a **peer export format** via `conspectus dump --format json`, but
JSON is not on the warm-start read path.

### File layout

- Database file: `$XDG_DATA_HOME/conspectus/graph.sqlite`.
- WAL sidecar: `$XDG_DATA_HOME/conspectus/graph.sqlite-wal`.
- Shared-memory sidecar: `$XDG_DATA_HOME/conspectus/graph.sqlite-shm`.
- Numbered backup directory: `$XDG_DATA_HOME/conspectus/backups/`.
  Used by the rotation policy below.
- Snapshot files never land inside project trees, per the existing
  design.md:657 invariant.

### Pragmas

Every connection applies the same triplet at open:

- `synchronous = NORMAL`
- `busy_timeout = 5000`
- `wal_autocheckpoint = 1000`

WAL mode itself is set once at database creation
(`journal_mode = WAL`) and persists across sessions.

### Atomicity

Atomicity is delegated to SQLite transactions. There is no
temp-file-plus-rename dance. Every mutation (cold rebuild, partial
provider eviction, declared-link CRUD, alias overlay update) runs
inside a `BEGIN; ... COMMIT;` block. Crash-mid-commit semantics are
SQLite's: the WAL replay path recovers the last fully-committed
transaction.

### Schema versioning

The schema version lives in `PRAGMA user_version` and is aligned
with the `GraphSnapshot` schema version constant in `src/model/`.
Migration applies forward-only. The v1 implementation uses
`rusqlite_migration` or a hand-rolled `user_version`-driven
applier; the choice is implementation detail recorded in P9-001's
spike outcome, not in this ADR.

Schema-mismatch policy:

- If the database's `user_version` is **lower** than the binary's
  expected version, run the migration chain forward.
- If the database's `user_version` is **higher** than the binary's
  expected version (i.e. the user downgraded), refuse to open and
  emit a clear error suggesting either upgrading the binary or
  removing the database file.
- If the migration chain fails part-way (e.g. disk full, file
  permission), abort the open and leave the database file
  untouched so the next attempt can resume.

### Per-provider freshness

A dedicated `provider_state` table records the per-provider refresh
timeline. Schema (subject to refinement in P9-002):

```sql
CREATE TABLE provider_state (
    provider        TEXT PRIMARY KEY,
    last_run_at     INTEGER NOT NULL,       -- Unix epoch seconds
    last_outcome    TEXT NOT NULL,           -- 'success' | 'error' | 'skipped'
    last_error      TEXT                     -- nullable detail
);
```

Each `node`, `candidate_link`, and `resolved_relationship` row also
carries a `provider TEXT NOT NULL` column referencing the producing
provider's stable identifier. This is the data foundation for
partial eviction (below) and shared with the in-memory
`GraphSnapshot` work in `P7-002` (the provider-provenance story).

### Partial eviction

Re-running a single provider's discovery slice is one transaction:

```sql
BEGIN;
DELETE FROM nodes               WHERE provider = ?;
DELETE FROM candidate_links     WHERE provider = ?;
DELETE FROM resolved_relationships WHERE provider = ?;
INSERT INTO nodes               (..., provider) VALUES (...);
INSERT INTO candidate_links     (..., provider) VALUES (...);
-- resolver re-runs after the merged candidate set is fresh
UPDATE provider_state SET last_run_at = ?, last_outcome = ?, last_error = ? WHERE provider = ?;
COMMIT;
```

The resolver runs in Rust (per ADR 0041) after the candidate-link
table is updated, then writes its `resolved_relationships` rows back
into the same transaction or a follow-up transaction. Concurrency:
WAL allows readers to keep querying the prior snapshot while this
transaction is in flight; no reader blocks.

### Rotation and backups

SQLite's `VACUUM INTO 'backups/graph-<epoch>.sqlite'` produces a
point-in-time backup file without blocking writers. The rotation
policy:

- After each successful full rebuild (cold start with all providers
  fresh), produce a `VACUUM INTO` backup.
- Keep the most recent **N=5** backups; delete older ones.
- Backups are **debugging artifacts**, not part of the warm-start
  read path. The warm-start path always reads `graph.sqlite`
  directly.

### `--no-cache` / `--refresh` flag interaction

- `--refresh` forces a cold rebuild for all providers regardless of
  TTL. The result still lands in `graph.sqlite`; the flag only
  bypasses warm-start logic on the way in.
- `--no-cache` disables persistence for this invocation entirely: a
  fresh in-memory snapshot is built and discarded at process exit.
  No write to `graph.sqlite`. Useful for debugging and for scripts
  that explicitly do not want to mutate the persistent store.
- These flags surface on every command that reads the graph
  (`session`, `table`, `node show`, `query`, the TUI).

## Consequences

- The on-disk artifact for graph state becomes a SQLite file. JSON
  remains hand-inspectable through `conspectus dump --format json`
  but no longer holds the warm-start contract.
- The temp-file-plus-rename atomic write pattern goes away,
  replaced by SQLite transaction semantics. The crash-recovery
  surface area shrinks because there is no half-written rename
  state to reconcile.
- Schema migrations become a first-class concern. Each
  model-shape change (e.g. `P7-002`'s provider-provenance fields)
  must ship a migration alongside the Rust model change. The
  migration chain is forward-only; older binaries cannot open
  databases written by newer ones, which is the safe default.
- Partial eviction (`P7-005`) becomes a SQL transaction rather
  than a separate merge primitive. Its implementation simplifies
  significantly.
- Server / one-shot CLI coexistence — settled in ADR 0038 — is
  built on top of this persistence model. Readers open
  `graph.sqlite` in read-only mode and benefit from WAL's
  concurrent-reader guarantees; the server holds the writer
  connection when running.
- `docs/design.md` §"Graph Snapshot Persistence" (lines 649–692)
  needs rewriting. That rewrite is a follow-up landing-doc task,
  not part of this ADR.
- The `H-PROD-002` hardening item (cache layer for forge metadata,
  tmux, harness scans) folds naturally into this model: per-
  provider caches become rows in `provider_state` or sibling
  tables in the same database file. Coordinated in the P7-002 +
  P9-002 timeframe.

## Alternatives Considered

- **JSON canonical, SQLite cache (the "B3 hybrid" from the prior
  plan draft).** Rejected. Two canonical stores create
  invalidation logic, mtime races, and a "rebuild on read"
  performance pothole that the warm-start path is supposed to
  avoid. The hybrid was the right answer under a DuckDB engine
  (where the single-writer file lock forced workarounds) but
  loses its raison d'être once SQLite's WAL mode handles the
  concurrency story directly.
- **SQLite canonical with no JSON export.** Rejected because
  `docs/design.md` line 660 names hand-inspectability as a
  property of the snapshot format. JSON-as-export preserves that
  property without paying for it on the warm-start path.
- **Multiple SQLite files (one per provider).** Rejected. Cross-
  provider joins in the resolver become `ATTACH DATABASE` dances;
  partial eviction transactions span multiple files; backup
  rotation multiplies. The single-file model is simpler at this
  scale.
- **In-place migration on every schema change.** The chosen
  policy (forward-only migrations, with refusal-to-open on
  downgrade) is the safer default. Reversible migrations may be
  considered case-by-case once the schema stabilizes; for now,
  forward-only matches the implementation effort warranted by an
  early-stage tool.

## Open Questions Answered

- **What is the wire format for the JSON export?** The same
  versioned JSON document `docs/design.md` line 658 already
  describes — `GraphSnapshot` plus a schema version plus a
  per-provider freshness map. The export reads from SQLite and
  renders the JSON; the JSON is no longer the source of truth.
- **How are aliases (ADR 0029) handled?** The `aliases` table in
  the schema mirrors the alias overlay. The alias overlay's
  read/write surface (currently `src/aliases.rs`) gains a SQLite
  backend when the `query` feature is on; without it, the file-
  based overlay continues to work and the database table is
  rebuilt from the file on cold start. This keeps the alias
  surface working in both build configurations.
- **What about declared links (ADR 0014)?** Declared links are
  read-once at discovery time and converted into candidate links.
  They land in `candidate_links` via the loader like everything
  else. The TOML files under project / workspace roots remain
  the durable authoring surface.
- **How does this interact with continuous server mode?**
  ADR 0038 covers that. In one sentence: the server holds the
  writer connection when running; one-shot CLI invocations open
  the database in read-only mode and use WAL's concurrent-reader
  semantics for queries; mutation commands either route through
  a Unix socket to the server or take the writer lock directly
  when the server is absent.
- **What about WAL across a NFS/SMB mount?** Not supported,
  consistent with ADR 0036 §"Open Questions Answered" and
  `docs/design.md` line 643. The conspectus model is single-user,
  single-machine.
- **Does this require a migration for existing users?** Conspectus
  is pre-1.0 and the prior on-disk format was never specified
  beyond the JSON sketch in `docs/design.md`. The first release
  that ships P9-003 (the loader) treats an absent database as a
  cold start; there is no prior corpus to migrate from.
