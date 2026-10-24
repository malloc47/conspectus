# ADR 0146: The Hook Spool Is The Durable Hook Record

## Status

Accepted. Refines ADR 0028 (hook sidecars) and the daemon hook ingest
path from the graph snapshot migration; stays inside ADR 0143
category 2.

## Context

`conspectus hook write` sends each record to a running daemon's
`hook_ingest` command and writes `hooks-latest.json` (the spool) only
when no daemon answers. The design said the daemon then persists hook
evidence through `graph.bin`.

It didn't. The hook-sidecar pass is an always-rerun mutator: every
class refresh evicts its links and replays it from the spool. A record
that only reached the daemon's live graph was gone by the next cycle,
within about five seconds. While the daemon ran, the spool never
changed, so harness hooks did nothing that lasted, and sessions fell
back to process-tree matching. On the operator machine the spool was
last written three weeks before this was found, and a live session sat
as a candidate in two tmux sessions instead of being tied to its own
pane.

## Decision

1. **The spool is the durable hook record, with or without a daemon.**
   `hook_ingest` writes the record to the spool before it updates the
   live graph, under the writer lock, so ingests are serialized and a
   refresh replays the record like any other.
2. **The daemon writes the spool it reads.** The record goes to the
   hook-sidecar root of the daemon's own discovery config, not the
   root the hook process would pick from its environment, so the writer
   and the replaying reader always agree.
3. **A failed spool write refuses the ingest.** The daemon answers
   `hook_spool_write_failed` and leaves the graph alone; the client
   warns and writes the spool itself, as it does for any daemon error.
4. **`graph.bin` is not a hook store.** It carries hook-derived links
   between a cycle and the next only as a published snapshot does.

## Consequences

- Hook attribution survives refreshes and daemon restarts. A restarted
  daemon replays the spool on its first cycle.
- The daemon gains no new write path: hook sidecars are already
  category 2 in ADR 0143, which lets the daemon write them.
- With a daemon running, hook processes no longer write the spool, so
  two harnesses starting at once can't lose one another's record in the
  spool's read-modify-write. Without a daemon that race is unchanged.
- The latest-per-mux spool keeps its size bound; stale records are
  still ignored for active attribution.

## Alternatives Considered

- **Have `hook write` always write the spool, then notify the daemon.**
  Equally durable and keeps the daemon from writing, but every hook
  process does the read-modify-write, unserialized, and it writes the
  root the hook's environment names, which can differ from the daemon's.
- **Keep ingested records in daemon memory and replay them each cycle.**
  Survives refreshes but not a restart, and adds a second hook store
  beside the spool.
- **Exempt hook-sidecar links from refresh eviction.** Stale or
  superseded records would never leave the graph, and the pass would no
  longer be a pure function of its inputs.
