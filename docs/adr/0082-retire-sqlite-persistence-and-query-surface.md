# ADR 0082: Retire SQLite Persistence And Query Surface

## Status

Accepted

## Context

ADRs 0036, 0037, 0038, 0039, 0040, 0042, 0043, and 0044 stand up
SQLite as Conspectus's canonical persisted store, IPC-free read
surface (via WAL), and user-facing query engine
(`conspectus query <sql>`). At the time those ADRs landed the
shape was driven by three reinforcing assumptions:

1. Persistence had to be the warm-start read path because a fresh
   one-shot `conspectus session` invocation could not afford a
   full discovery cycle.
2. A user-facing SQL surface was a primary product feature whose
   ergonomics justified an embedded engine with a queryable schema
   that mirrored the in-memory model.
3. SQLite's WAL gave a "many readers, one writer" coexistence
   story that let the daemon and one-shot CLIs share data without
   bespoke IPC.

Phase 7 then introduced `conspectus serve` (ADRs 0038, 0080) with
a per-class scheduler, signal-driven shutdown, and event-driven
refresh via `notify` (ADR 0081). Layer A/B/C of P7-006 plus P7-009
shipped, and the daemon proved out three things that invalidate
the original assumptions in conspectus's actual usage envelope:

- **Cold discovery is fast.** On developer hardware in the target
  workload (single-digit harness state dirs, dozens to hundreds of
  graph nodes), a full rebuild is single-digit seconds end-to-end.
  A warm-cache hit is not measurably faster than cold for a TUI
  launch.
- **The daemon is the natural source of truth.** With per-class
  refresh, watcher-driven harness updates, and a process-local
  writer mutex, the daemon already holds a fully-hydrated
  `GraphSnapshot` in memory and re-derives it on its own schedule.
  Routing that through SQLite — serialize to rows, deserialize
  back to typed nodes via `query::reader` on every read — pays a
  marshalling cost that did not exist when the snapshot was a
  Rust value.
- **The SQL query surface is a nice-to-have, not load-bearing.**
  Real conspectus workflows are CLI projections (`table`,
  `session`, `node show`), the TUI, and `graph` exports. The
  `conspectus query <sql>` feature is convenient but accounts for
  almost no operator-visible use, while `src/query/` is the
  largest single module in the crate (~6500 lines across loader,
  reader, schema, runner, persistence). The schema mirror exists
  *because* of the SQL surface, not because the consumers need
  it.

These observations make the SQLite cluster a heavy implementation
choice for a problem conspectus no longer has. The opportunity
cost shows up in: model evolution friction (every schema change
needs a paired migration + loader/reader update + drift test),
dependency weight (bundled libsqlite3 dominates compile time and
binary size), distribution constraints (ADR 0040's amendments
exist specifically to accommodate the bundled engine), and
cognitive load for contributors who must understand the
producer/consumer split across in-memory + SQLite forms.

This ADR pivots the architecture to make the daemon the primary
read surface, with a small on-disk artifact for daemonless reads.
The companion ADR 0083 settles the wire format for that artifact.

## Decision

Retire SQLite as a persistence layer and as a user-facing query
engine. Replace it with:

1. The daemon as the canonical live source of truth. It holds a
   fully-hydrated `GraphSnapshot` in memory and serves reads
   directly to in-process consumers (TUI bound to the daemon) and
   over the existing Unix-domain socket (to one-shot CLIs that
   want fresh data without paying cold-discovery cost).
2. A single on-disk snapshot artifact written by the daemon after
   each successful cycle: an atomically-renamed binary blob in a
   zero-copy format (ADR 0083 picks the format). Readers `mmap`
   it; daemonless one-shot CLIs use it directly. There is no
   schema migration chain — a version-header mismatch triggers a
   cold rebuild rather than an in-place upgrade.
3. Removal of the `conspectus query <sql>` subcommand, the
   `query` Cargo feature, the entire `src/query/` module, the
   `rusqlite` dependency, the bundled libsqlite3 build, and the
   `MIN_SQLITE_VERSION` floor.
4. Removal of the mutation socket's writer-lock-fallback story.
   With no shared on-disk database, "daemon absent" mutations
   are local file edits (declared links, aliases, pins) that
   continue to live in TOML per ADRs 0014, 0029, 0057 and do not
   need IPC.

### What the daemon owns

- **In-memory `GraphSnapshot`** behind an `ArcSwap` (or
  `RwLock<Arc<…>>`) so reader threads see a coherent snapshot
  without taking a lock that contends with the writer.
- **Per-class refresh** as today: each class thread evicts its
  slice, re-runs discovery, resolves, and `ArcSwap::store`s the
  new snapshot.
- **On-disk artifact** written after each successful cycle. The
  format is chosen in ADR 0083; this ADR fixes only that:
  - A 16-byte header carries `magic`, `version`, and
    `payload_len`.
  - The file is written to `graph.tmp`, fsynced, then
    `rename`d to `graph.snapshot` (path-name TBD by ADR 0083).
    POSIX rename atomicity guarantees concurrent readers
    holding the old inode keep seeing the old file until they
    drop the mapping.
  - File size at conspectus's target scale (single-digit MB
    even for graphs an order of magnitude larger than today's)
    makes per-cycle write cost negligible on any modern SSD.
- **Read API on the socket** for in-flight callers that want the
  newest data without re-reading the file: a `snapshot` command
  that returns the current snapshot's bytes (the same bytes the
  daemon just wrote to disk, served from an `ArcSwap<Bytes>`
  cache).

### What readers do

- **TUI when daemon is present**: connects to the socket, holds a
  long-lived connection, requests `snapshot` on each refresh
  tick. Sub-millisecond round-trip; no per-frame disk hit.
- **TUI when daemon is absent**: cold-builds the snapshot
  in-process. With the new format this is a `discover →
  resolve → write tmp + rename` flow ending in a usable
  in-memory snapshot, and (optionally) a written file the next
  invocation can mmap.
- **One-shot CLI (`session`, `table`, `node show`, `graph`)**:
  prefers the socket when present; otherwise mmaps the on-disk
  snapshot if recent enough; otherwise cold-builds. "Recent
  enough" is a TTL check against the class intervals so a stale
  artifact does not silently shadow a real change. Daemonless
  cold rebuild remains the structural safety net for the
  "absence is not an error" guarantee.
- **External consumers** (scripts, future integrations) get the
  same `mmap` path as the CLI, or pipe through
  `conspectus graph --format json` for a hand-inspectable export.
  The JSON dump remains the documented inter-tool contract.

### What goes away

- `src/query/` in its entirety: `schema.rs`, `schema.sql`,
  `loader.rs`, `reader.rs`, `persist.rs`, `runner.rs`, `mod.rs`,
  and the in-module test fixtures. The two integration test
  binaries dedicated to the query path (`tests/cli_query.rs`,
  `tests/query_regression.rs`) follow.
- The `conspectus query` subcommand, `--list-views`,
  `--similar-to`, `--load-extension`, the `OutputFormat::{Table,
  Json, Csv, Tsv}` query-specific variants, the saved-view
  library, and the embedding overlay table.
- The `conspectus dump --format json` write path stays (now
  driven directly from the in-memory `GraphSnapshot`); the
  read-from-SQLite branch goes away.
- The `--no-cache` / `--refresh` flag semantics simplify:
  `--refresh` means "ignore any on-disk artifact and cold-
  rebuild"; `--no-cache` means "do not write the artifact for
  this invocation." The daemon's `refresh` socket command stays.
- ADR 0037's rotation / backup / `VACUUM INTO` / schema-version
  migration story. Replaced by "current snapshot or rebuild."
- ADR 0042's vector-search surface and the embedding ingestion
  follow-up (P9-FU-001). Reopen as a separate workstream if the
  feature comes back.
- The mutation-socket writer-fallback contract from ADR 0038's
  §"Write path". TOML mutations are local file edits; refresh
  is the only write the daemon needs to coordinate. Future
  per-mutation socket routing can return scoped to specific
  commands when justified.

### What stays

- TOML-rooted user-authored state: `.conspectus.toml` for
  declared links (ADR 0014), aliases (ADR 0029), session pins
  (ADRs 0057, 0058). These were never SQLite-canonical — SQLite
  mirrored them. The mirror disappears; the TOML source of truth
  stays.
- ADR 0080 (daemon signal handling) and ADR 0081 (filesystem
  watcher dependency) carry through unchanged. The daemon
  lifecycle is unaffected.
- The Unix-domain socket transport from ADR 0038, length-prefixed
  JSON framing, the `ping` / `refresh` / `status` commands. The
  socket gains a `snapshot` read command (specified above) and
  loses its writer-fallback fork.
- The producer pipeline (`discovery → resolve → GraphSnapshot`)
  is unchanged. `GraphSnapshot` returns to its original role as
  the singular in-process graph representation.

### Migration shape

The retirement is staged so the change is reviewable and the
build stays green at each step:

1. ADR 0083 lands and picks the on-disk format.
2. Model types gain whatever derives the format needs (one
   commit, mostly mechanical).
3. New snapshot writer + reader land alongside the existing
   SQLite path, behind a feature flag or runtime selector.
4. Daemon writes both formats per cycle for one release cycle.
5. The socket `snapshot` command lands; TUI is cut over to
   prefer the socket when present.
6. One-shot CLIs cut over to mmap-or-rebuild; the SQLite read
   path is deleted.
7. `conspectus query` is removed; the `query` feature flag is
   removed; `src/query/` is deleted; `rusqlite` is dropped.
8. ADRs 0036, 0037, 0038 (partial — write path), 0039, 0040,
   0042, 0043, 0044 marked superseded with pointers here.
9. `docs/design.md` §"Continuous Operation Mode" and §"Graph
   Snapshot Persistence" rewritten against the new model.

The detailed story breakdown lives in `docs/backlog.md` Phase 11.

## Consequences

- The producer/consumer split collapses. `GraphSnapshot` is the
  single in-process graph representation again. Adding a field to
  a model type no longer requires touching a schema, loader,
  reader, drift test, and migration.
- Cold one-shot CLI invocations without a daemon cost the full
  discovery time (seconds on the target workload). This is the
  load-bearing tradeoff of the decision and the reason the
  empirical "cold rebuild is fast" observation needed to come
  first. If discovery time regresses materially for a future
  workload, the answer is to make the daemon mandatory for that
  workload, not to reintroduce a query-engine-shaped cache.
- The release-binary size drops substantially (bundled SQLite is
  one of the largest single contributors). Clean-build time
  drops by the libsqlite3 compile.
- The `conspectus query` feature disappears. Power users who
  relied on it must move to `conspectus graph --format json |
  jq` or wait for a replacement feature (out of scope here).
  Given the observed usage, this is acceptable; reopening a
  query surface — likely against the JSON export rather than an
  embedded engine — is a separate ADR if the demand resurfaces.
- ADR 0040 (distribution amendment for bundled libsqlite3) is
  superseded. ADR 0016's original "single static binary, no
  runtime deps" property is reinforced.
- The daemon stops being optional in a soft sense: TUI users who
  want sub-second refresh will run the daemon, and one-shot CLIs
  in a freshly-cloned repo will pay the cold-rebuild cost. This
  is consistent with how `git status` behaves in a large repo:
  fast when the cache is warm, slower on first invocation, never
  silently wrong.
- Schema evolution becomes "bump the version constant in the
  format module; recompile; the daemon writes a new file; old
  files are ignored on read." No migrations, no version-fork
  reasoning, no refusal-to-open ceremony.
- The mutation socket simplifies. ADR 0038's "writer fork"
  detail is retired; the socket is reads + refresh + status.
  Future commands that need server-side coordination land as
  new commands; the protocol shape (length-prefixed JSON) is
  untouched.

## Alternatives Considered

- **Keep SQLite, drop only the query surface.** Removes the
  largest single piece of code (`runner.rs` + saved views) but
  leaves the schema/loader/reader/persist machinery in place
  for what becomes a glorified key/value store. The
  marshalling cost on every read survives; the dependency
  survives; the migration story survives. Saves ~25% of the
  code at ~5% of the architectural simplification.
- **Keep SQLite, replace the daemon's writes with an in-memory
  ArcSwap and treat SQLite as a downstream consumer cache.**
  Restores producer/consumer separation but means the daemon's
  reads-from-self path also has to deserialize from SQLite —
  no actual win. Variant: have the daemon hold the snapshot in
  memory and only write SQLite for CLI consumption. Better,
  but every "SQLite for consumption" change still requires
  schema, loader, reader, drift test. Half the cost, half the
  win.
- **Keep SQLite but make it ephemeral (rebuilt on daemon
  startup).** Removes schema migrations; keeps everything
  else. Solves the smallest problem in the cluster while
  retaining the dependency weight and marshalling cost.
- **Swap SQLite for an embedded KV store (`sled`, `redb`).**
  Trades one heavy dep for another while keeping the
  producer/consumer split. Does not solve the marshalling cost
  unless paired with a zero-copy value format anyway — at
  which point the KV layer is wasted overhead on top of the
  format that's doing the actual work.
- **Replace SQLite with a JSON file (today's
  `graph-snapshot.json`).** Simpler still; loses the "no
  deserialize on read" property entirely. Acceptable if cold
  rebuild + JSON parse is fast enough, but pays the parse cost
  on every reader. ADR 0083 evaluates this as one of the
  format options.
- **Keep everything, deprecate nothing.** Tempting because the
  code works. Rejected because the carrying cost (model
  evolution friction, dependency weight, distribution
  constraints, schema migrations on every model change, code
  surface area) is what this ADR is actually optimizing for.
  The functional capability the code provides is not load-
  bearing for any current workflow.

## Open Questions

- **Should the on-disk artifact write be opt-out?** A daemon
  running on a read-only filesystem (rare but possible in
  container deployments) should not crash. Treat write failures
  as non-fatal warnings, log once, continue. The socket read
  path still works for any connected TUI.
- **How long should a one-shot CLI trust an on-disk artifact
  without a daemon present?** A natural answer is "as long as
  the slowest class interval" (forge = 5 minutes today). Older
  than that, force a cold rebuild. Decision deferred to the
  P11 story that ships the mmap-or-rebuild reader.
- **What happens to operators currently running `conspectus
  query` in scripts?** Documented breakage. A migration note
  in `docs/operations.md` and the next release's CHANGELOG
  points them at `conspectus graph --format json | jq`. If a
  replacement query surface lands later it is a separate ADR.
- **Does the daemon ever need to read its own on-disk
  artifact?** Only on startup, as a warm-start optimization to
  avoid paying a full discovery cycle before the first read
  arrives. Worth implementing iff the cold-start cost is
  user-visible in normal operation; skip otherwise.
- **Per-mutation socket routing for declared/alias/pin
  commands?** Out of scope. Those continue to write TOML
  files; the daemon's filesystem watcher (or the next refresh
  tick) picks up the change. If contention or ordering
  becomes a real problem, a future ADR can route through the
  socket.
