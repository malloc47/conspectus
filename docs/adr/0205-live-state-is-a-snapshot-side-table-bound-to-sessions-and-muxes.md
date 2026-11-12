# ADR 0205: Live State Is A Snapshot Side Table Bound To Sessions And Muxes

## Status

Accepted. Part of the live agent state plan with ADRs 0203, 0204, 0206,
and 0207. Extends the graph snapshot (ADR 0083) with two sparse tables,
following the presence table of ADR 0140. Bumps the rkyv format version.

## Context

ADR 0203 defines the observation and ADR 0204 stores it. The graph
needs to carry it to every consumer: the TUI row builders, `conspectus
table`, `graph --format json`, `node show`, and the library API (ADR
0015).

The graph has no pane node and no per-node attribute bag. Metadata
exists only on links, and the process-tree fingerprint (ADR 0091) zeroes
volatile node fields so that churn does not reopen the expensive `/proc`
walk. The research audit
(`docs/plans/live-state/research/06-internal-audit.md` §2.4) compared
five representations:

| | Consequence of choosing it |
|---|---|
| A. Fields on `AgentSessionNode` and `MuxSessionNode` | smallest change; churns identity nodes, must be zeroed in the fingerprint, breaks 31 struct literals in the library API, covers the active pane only |
| B. An observation node kind | right if panes become structure; ids that embed time churn; every exhaustive match changes |
| C. Evidence links | a state is not a relationship; `ResolvedRelationship.target` is a node id |
| D. A sparse side table on the snapshot | follows `node_presence`; stable identity and fingerprint; small serialized payload; adds one field to `GraphSnapshot` |
| E. Outside the graph | invisible to JSON and the library, against the machine-readable-graph requirement |

## Decision

1. **`GraphSnapshot` gains `node_live: BTreeMap<NodeId,
   LiveBinding>` and `pane_live: BTreeMap<LivePane, LiveBinding>`**,
   sparse like `node_presence`: an absent key means no evidence. A
   `LiveBinding` is the stored `LiveObservation` and the `Liveness` its
   binder found for the observation's process (decision 4). A
   `LivePane` is a tmux mux's id and a pane id (`%3`, or none when the
   entry named no pane). Both ride in `graph.bin` and serialize to JSON
   as lists (as `node_presence` does), each `node_live` entry the node
   id beside the observation's fields and `liveness`, and each
   `pane_live` entry the mux's node id and `pane` beside the same
   fields. Each is omitted when empty so state-less goldens do not
   change.

2. **`node_live` keys are `AgentSession` ids; `pane_live` keys are a
   mux and a pane, for pane-only entries.** Binding (the store's
   entries to nodes) follows three rules, in order:
   1. An entry binds to the `AgentSession` whose harness and native
      session id match. The match is exact. Two session nodes for one
      native session (the same id under two state scopes) bind the
      lower node id.
   2. Otherwise, an entry with a tmux socket and session name binds to
      the pane it names on the tmux `MuxSession` it names, in
      `pane_live`. This covers a new session that has no transcript yet
      (Pi after `/new`, Codex before the first rollout). A mux holds
      counts over these and over its hosted sessions. The server is the
      socket path's last component, the name `tmux -L` gives it, with
      `default` for the default server, matched against the mux node's
      socket.
   3. Otherwise it stays in the store, unbound, and appears nowhere.
      Live state alone never creates an agent session node (the graph
      is sparse; the hook sidecar already synthesizes sessions where
      attribution needs one).

   Only nodes that exist now take live state: a gone or unknown node
   from history (ADR 0140) binds nothing. A session holds one entry,
   and so does a pane: the graph has no pane node, so the pane id is a
   sub-key of its mux, and two agents in two panes of one mux that no
   session matches yet each count on it. When several entries bind to
   one session or one pane, it keeps the one the store wrote last: the
   highest entry `rev`, then the newest `observed_ms`, then one that has
   not exited (a process that moves to a new session ends its old entry
   in the same write), then the lowest entry key. Entries that name no
   pane share one slot on their mux.

3. **A pure function does the binding**: `bind_live(snapshot, file,
   probe) -> BoundLive` (both tables) in `discovery::live_state`,
   over the store's file and a `PidProbe` (decision 4). It takes no
   clock: time enters only when a reader computes the effective state.
   Two callers use it:
   - the discovery mutator `live_state` (a `Mutator` provider beside
     `hook_sidecar`, reading the store directory in
     `LocalDiscoveryConfig::live_state_root`), which runs with the other
     mutators after the harness providers and the hook sidecar (daemon
     class cycles, one-shot cold rebuilds, and the daemonless TUI's
     background discovery) and replaces both tables, so a snapshot
     always carries tables rebuilt from the store. It reads one file and
     checks one pid per bound entry, and it starts no process;
   - the TUI's overlay, which re-runs it
     against the snapshot it holds whenever the store changes, so state
     is visible within about a second without a rediscovery.

4. **Liveness comes from the process evidence discovery already has.**
   An entry's pid is checked against the process table and the recorded
   start time through a `PidProbe` (`/proc/<pid>` and field 22 of its
   `stat` on Linux), at bind time, and the answer is stored in the
   binding. Where the platform offers no cheap check, liveness is
   `Unknown` and the 15-minute rule applies (ADR 0203). A daemon
   snapshot's answer is at most one class cycle old. The TUI's answers
   are at most a second old: an agent killed without a goodbye writes
   nothing to the store, so between binds the overlay checks the
   processes of its bound entries again, at most once a second counting
   the check each bind makes. That costs one read of
   `/proc/<pid>/stat` per bound entry whose process may still run, reads
   no store, starts no process, and reads nothing when no entry is
   bound; an entry that recorded its exit, or whose process was already
   found gone, is not checked again. A process found gone or reused
   updates the rows in place, like a store rewrite.

5. **Effective state is read through an accessor**, never from the raw
   table: `snapshot.live_state_of(id, now)` returns the ADR 0203
   effective state of a session, read with the stored liveness; a mux
   has none. `snapshot.live_counts_of_mux(id, now)` returns
   `LiveCounts` over the mux's hosted sessions (found with the existing
   preferred-mux helpers) and every pane-only entry of the mux, each
   read the same way, and `snapshot.live_panes_of(id)` lists those
   entries with their panes. A reader needs no process check of its
   own. A hosted session counts when
   the mux is its preferred mux and it has an entry; one without an
   entry is not counted at all.

6. **Not in the fingerprint.** Neither table is part of the process-tree
   fingerprint and the fields of identity nodes are unchanged, so a state
   change never reopens the `/proc` walk (ADR 0091).

7. **Eviction and merge** treat the tables like presence: the mutator
   owns them, so they are rebuilt on every cycle, and a provider
   eviction drops entries for the nodes it owned, a mux's pane entries
   with the mux. A `GraphFragment` (provider output) has no live table;
   the conversion from a snapshot names them among the fields it leaves
   behind, and the warm-start merge keeps the fresh snapshot's tables
   and drops the prior's.

## Consequences

- Every output that serializes a snapshot can show live state without new
  plumbing, and the TUI reads it the same way it reads presence.
- `FORMAT_VERSION` bumps (to 12, which carries both tables); readers
  cold-rebuild as ADR 0082 describes.
  About 31 full `GraphSnapshot` literals in tests and the library need the
  new field (those that use `..empty()` do not).
- The table can be stale by one class cycle in a daemon snapshot (at most
  5 s). The TUI overlay covers the difference for the interactive case.
- A pane-only entry shows on the mux row, not on any session. If the
  session appears later, the next bind moves the state to it.
- Mux aggregation is computed, so the model does not need to decide which
  state a mux row shows (ADR 0203).

## Alternatives Considered

- **A (fields on nodes).** The simplest to read, and the wrong place to put
  something that changes every few seconds: it touches the identity nodes
  and the fingerprint, and a mux would hold only its active pane's state.
- **B (observation nodes).** Worth revisiting if panes become first-class.
  Until then it adds a node kind, a reverse index, and ids that churn.
- **C (links).** Would require a relation whose target is not an entity.
- **E (outside the graph).** Rejected for the machine-readable requirement;
  an in-memory overlay remains as the TUI's fast path in front of D.
- **A pane sub-key inside `node_live`** (a key of node id and optional
  pane). One list in JSON, but every reader and test of a session's
  entry would build the compound key, and the type would allow a pane
  on a session's key. A second table keeps `node_live` one entry per
  node and leaves session readers as they were.
- **Liveness supplied by each reader** (the accessors taking a closure
  from the observation to `Liveness`). Every reader would need a process
  table, a script reading `graph --format json` could not tell a dead
  agent's last state from a live one, and the binder checks the pid
  anyway. Storing the answer costs one field per entry.
- **Folding a dead process into the stored state** (rewriting the entry
  as `exited/killed` at bind time). It would lose the state the agent was
  in, and readers could not tell an unchecked process from a live one.

## Open Questions Answered

- *Does the daemon snapshot need to be fresher than 5 s?* Not for the
  interactive case, where the overlay is fresher. Scripts that want the
  freshest value can read the store through the CLI (a story adds a
  `--fresh` read that skips the snapshot).
- *Where is a split pane's state?* Each pane's agent has its own entry. The
  mux shows counts over all of them. A session's own row shows its own
  state. Until a pane's agent has a session node, its entry binds to its
  pane of the mux in `pane_live`, one entry per pane (decision 2).
