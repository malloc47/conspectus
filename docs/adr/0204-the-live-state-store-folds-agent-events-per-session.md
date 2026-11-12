# ADR 0204: The Live-State Store Folds Agent Events Per Session

## Status

Accepted. Part of the live agent state plan with ADRs 0203, 0205, 0206,
and 0207. Refines ADR 0028 and ADR 0146: attribution stays in the hook
spool; state gets its own store beside it. A category 2 observation
under ADR 0143, inside the manifest's `$STATE/conspectus/**` path.

## Context

The hook spool (`hooks-latest.json`, ADR 0146) keeps the latest record
per tmux session. That is enough for attribution and wrong for state:

- two agents in one tmux session (split panes) overwrite each other;
- timestamps have one-second resolution and writers do not lock, so
  events within a second land in any order;
- one record per key keeps no counters (subagents in flight, pending
  waits), no "since", and no history;
- the spool's only reader replays it on every discovery, so each record
  gets bound through the attribution rules, which a state does not need.

Hook-to-daemon ingest also costs a full resolve, a full rkyv
serialization, and a `graph.bin` rewrite, and the socket client has no
timeout. A state event can fire twice per tool call, so a state path
that rides on it would slow every agent and hang one on a stuck daemon.

Claude Code and Codex run a new `conspectus hook write` process per
event, so something has to hold the state between events. OpenCode and
Pi run a plugin inside the harness, which can hold it itself.

## Decision

1. **A separate store: `$XDG_STATE_HOME/conspectus/live-state.json`**,
   with `live-state.lock`. `{ "schema_version": 1, "rev": N, "entries":
   { "<harness>:<session_id>": Entry } }`. Writes are atomic (a sibling
   temporary file renamed into place) under an advisory lock, as the
   history store does (ADR 0140). The attribution spool is unchanged by
   this ADR.

2. **The unit is the entry**, one per `(harness, session_id)`. It holds
   a `LiveObservation` (ADR 0203), the cwd and transcript path, the tmux
   socket, session name, and pane id when known, an `instance` and `seq`
   for plugin sources, and a private `reducer` block (pending waits by
   agent id, active subagent ids, the state to resume after compaction).
   The `reducer` block is internal, may change between versions, and is
   not exposed through any output.

3. **Two ways in, one entry shape.**
   - *Event sources* (Claude Code, Codex): `hook write` maps the payload
     to a neutral `LiveEvent`, takes the lock, and folds the event into
     the entry with the harness's reducer.
   - *Snapshot sources* (OpenCode, Pi): the plugin sends the complete
     state of one session with `instance` and `seq`. The store replaces the
     entry when `instance` matches and `seq` is higher, or when `instance`
     is new and `observed_ms` is not older.

4. **Ordering is by store revision.** Every successful write takes the
   next `rev` under the lock, and entries record the `rev` and the
   millisecond time of their last change. Order is the order in which
   writers took the lock, which is the order the harness fired events
   within one session.

5. **One live entry per `(harness, pid)`.** An event with a new session
   id from a pid that already has an entry marks the old entry
   `exited/ended`. This covers `/clear`, `/new`, and resume, which swap
   the session in a running process.

6. **The writer is best-effort, bounded, and silent.**
   - The live-state write never waits on a daemon. For every event except
     a session start, `hook write` opens no socket. A session start
     still takes the existing attribution path (ADR 0146), after the
     live-state write, and that path gains a socket timeout.
   - It spawns tmux only to learn a session's socket and name the first
     time it sees the entry; later events reuse the stored pane
     information (`TMUX_PANE` is read directly).
   - The lock wait is 200 ms. On timeout the event is dropped.
   - It prints nothing to stdout or stderr and exits 0 whatever happens.
     Claude Code feeds some hook stdout to the model and treats exit 2
     as a block, so a bug here must not reach the agent.

7. **Garbage collection on write.** Entries in `exited` for more than an
   hour are dropped. Entries whose pid is dead and untouched for a day
   are dropped. The file keeps at most 512 entries (oldest first).

8. **Readers tolerate anything.** A missing, unreadable, or newer-schema
   file means no live state, never an error. Unknown fields in entries
   are ignored. A reader never writes the store.

9. **No payload.** Entries contain enums, counts, ids, paths, and times.
   The writer reads payloads in memory against a typed whitelist (ADR
   0206), and tests plant sentinel strings in every payload-bearing field
   and assert none reaches the file (ADR 0086 Tier 3).

## Consequences

- A hook event costs one small file write. The daemon, the graph, and
  `graph.bin` are not involved, and a daemon outage cannot slow an agent.
- State survives daemon restarts and works without a daemon (the TUI
  reads the same file), which keeps the one-shot CLI and the daemonless
  TUI equivalent.
- Two stores now describe the same hooks: the spool for attribution, the
  store for state. They share the harness events but not records. A
  `SessionStart` writes both.
- A mixed-version machine is safe: an older binary ignores the store,
  and a newer one ignores a store with a higher `schema_version`.
- Entries are keyed by session, so the pane-collision bug in the spool
  does not reach state. The spool's own fix is a separate story.
- The file is rewritten on every event, including heartbeats. At a few
  dozen entries that is a few kilobytes; the cost is small against the
  harness work being observed. If it ever matters, the store can shard by
  harness without changing the format of an entry.

## Alternatives Considered

- **Extend the spool with state fields.** Smallest change; inherits the
  per-tmux-session key, the second-resolution timestamps, and the
  replay-through-attribution path. Rejected.
- **An append-only event log, folded by readers.** Keeps history and makes
  readers pure, but needs rotation, partial-line handling, and a fold on
  every read (ADR 0028 rejected it for the same reasons). A fold in the
  writer keeps readers trivial. Rejected.
- **Daemon-held state with a `live_state` socket command.** Fast to read
  and a natural place for debouncing, but it makes the daemon a
  requirement for state and puts a socket on the hook's critical path.
  Rejected; a command can be added later over the same store.
- **SQLite.** Retired for hooks by ADR 0082. Rejected.
- **One file per session.** Avoids a single hot file, at the cost of a
  directory scan per read and many tiny files to collect. Rejected for
  now; the format of an entry would carry over.

## Open Questions Answered

- *What happens to events that arrive while the lock is held?* They wait
  up to 200 ms, then are dropped. A dropped event is repaired by the next
  one (every reducer is idempotent) or, for a working state, by the quiet
  window.
- *What if the pid cannot be found?* The entry has no pid; its liveness is
  `Unknown`, and the 15-minute rule applies (ADR 0203).
- *What about two machines sharing a home directory?* Out of scope; the
  pid and tmux socket make entries machine-local, and the store lives
  under `$XDG_STATE_HOME`, which is per machine by convention.
