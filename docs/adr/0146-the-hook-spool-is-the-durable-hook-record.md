# ADR 0146: The Hook Spool Is The Durable Hook Record

## Status

Accepted. Refines ADR 0028 (hook sidecars) and the daemon hook ingest
path from the graph snapshot migration; stays inside ADR 0143
category 2. Amended 2026-11-12: one record per pane, no time limit,
and an ingest that supersedes the pane's older link.

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

## Amendment: one record per pane, and no time limit (2026-11-12)

The spool kept the latest record per tmux session, so two agents in
split panes of one session overwrote each other's attribution, and a
daemon ingest left two Active hook links for one pane until the next
cycle. The docs also still promised a 15-minute limit that nothing
applied. Everything above stands except as follows.

1. **One record per pane.** A record's slot is derived from the record:
   `tmux:{socket}:{session}:pane:{pane}` for a pane of a mux session;
   `tmux:{socket}:{session}` for a mux record that names no pane, which
   attribution already treats as one per session; and outside a mux,
   the harness process (`pid:{pid}`), else the directory (`cwd:{cwd}`),
   else the session. A newer record replaces an older one in its slot,
   as before.
2. **Records under the old key migrate on read.** Readers never trust
   the stored key: they derive each record's slot and keep the newest
   per slot (on a tie, the record already stored under its slot), so a
   spool written under the per-session key reads correctly, and the
   next accepted write stores every record under its derived key. The
   schema version stays 1, so a binary from before this amendment reads
   the spool and adds its own record under the per-session key without
   dropping any other.
3. **A newer spool is never overwritten.** A spool whose schema version
   is higher than the binary's reads as empty and refuses writes. A
   later schema change can then bump the version without an older
   binary on `PATH` wiping the spool, which the plugins spawn. A missing
   or corrupt spool is still replaced by the next write.
4. **A size bound.** Per-pane slots grow with every pane a hook ever ran
   in, so the spool keeps at most 256 records and a write past that
   drops the ones observed longest ago. The cap only matters for an
   agent that reported nothing while 256 other panes or processes did,
   and its pane falls back to process evidence.
5. **No time limit.** No time-based TTL applies to hook records, and the
   unused 15-minute constant is gone. ADR 0028's v1 limit was removed
   from attribution because it dropped the links of idle but live
   sessions. A record stays evidence until a fresher record for its pane
   supersedes it. Attribution ignores it when its process is no longer
   running, when its mux was created after it, or when the mux's active
   pane runs another harness. That is what "stale" means in the
   consequences above.
6. **An ingest supersedes the pane's older link.** The hook-sidecar pass
   seeds its per-pane winners from the graph's Active hook links. An
   ingested record that wins its pane overrides the older link and
   drops its process observation, and one older than the graph's link
   is overridden by it, which is what the next cycle replays. A cycle
   runs the pass on a graph whose hook links were evicted, so it is
   unchanged.

Alternatives considered: replaying the whole spool on every ingest,
which would evict the pass's sessions and drop the live-state entries
bound to them until the next cycle; a liveness prune of records whose
process is gone, which can't bound records without a pid (every record
on hosts without `/proc`); and a schema version bump, which an older
binary would answer by replacing the spool with its one record.
