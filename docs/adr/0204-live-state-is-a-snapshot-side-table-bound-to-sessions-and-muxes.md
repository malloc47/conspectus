# ADR 0204: Live State Is A Snapshot Side Table Bound To Sessions And Muxes

## Status

Accepted. Part of the live agent state plan with ADRs 0202, 0203, 0205,
and 0206. Extends the graph snapshot (ADR 0083) with one sparse table,
following the presence table of ADR 0140. Bumps the rkyv format version.

## Context

ADR 0202 defines the observation and ADR 0203 stores it. The graph
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
   LiveObservation>`**, sparse like `node_presence`: an absent key means
   no evidence. It rides in `graph.bin` and serializes to JSON as a list
   (as `node_presence` does), omitted when empty so state-less goldens
   do not change.

2. **Keys are `AgentSession` ids, or `MuxSession` ids for pane-only
   entries.** Binding (the store's entries to nodes) follows three
   rules, in order:
   1. An entry binds to the `AgentSession` whose harness and native
      session id match. The match is exact.
   2. Otherwise, an entry with a tmux socket and session name binds to
      the `MuxSession` it names. This covers a new session that has no
      transcript yet (Pi after `/new`, Codex before the first rollout).
      A mux holds counts over these and over its hosted sessions.
   3. Otherwise it stays in the store, unbound, and appears nowhere.
      Live state alone never creates an agent session node (the graph
      is sparse; the hook sidecar already synthesizes sessions where
      attribution needs one).

3. **A pure function does the binding**: `bind_live(snapshot, entries,
   liveness, now) -> BTreeMap<NodeId, LiveObservation>`. Two callers use
   it:
   - the discovery mutator `live_state`, which runs with the other
     mutators after the harness providers and the hook sidecar (daemon
     class cycles, one-shot cold rebuilds, and the daemonless TUI's
     background discovery), so a snapshot always carries the table;
   - the TUI's overlay, which re-runs it
     against the snapshot it holds whenever the store changes, so state
     is visible within about a second without a rediscovery.

4. **Liveness comes from the process evidence discovery already has.**
   An entry's pid is checked against the process table and the recorded
   start time. Where the platform offers no cheap check, liveness is
   `Unknown` and the 15-minute rule applies (ADR 0202).

5. **Effective state is read through an accessor**, never from the raw
   table: `snapshot.live_state_of(id, now)` returns the ADR 0202
   effective state. `snapshot.live_counts_of_mux(id, now)` returns
   `LiveCounts` over the mux's hosted sessions (found with the existing
   preferred-mux helpers) and its pane-only entries.

6. **Not in the fingerprint.** `node_live` is not part of the process-tree
   fingerprint and the fields of identity nodes are unchanged, so a state
   change never reopens the `/proc` walk (ADR 0091).

7. **Eviction and merge** treat the table like presence: the mutator owns
   it, so it is rebuilt on every cycle, and a provider eviction drops
   entries for the nodes it owned.

## Consequences

- Every output that serializes a snapshot can show live state without new
  plumbing, and the TUI reads it the same way it reads presence.
- `FORMAT_VERSION` bumps; readers cold-rebuild as ADR 0082 describes.
  About 31 full `GraphSnapshot` literals in tests and the library need the
  new field (those that use `..empty()` do not).
- The table can be stale by one class cycle in a daemon snapshot (at most
  5 s). The TUI overlay covers the difference for the interactive case.
- A pane-only entry shows on the mux row, not on any session. If the
  session appears later, the next bind moves the state to it.
- Mux aggregation is computed, so the model does not need to decide which
  state a mux row shows (ADR 0202).

## Alternatives Considered

- **A (fields on nodes).** The simplest to read, and the wrong place to put
  something that changes every few seconds: it touches the identity nodes
  and the fingerprint, and a mux would hold only its active pane's state.
- **B (observation nodes).** Worth revisiting if panes become first-class.
  Until then it adds a node kind, a reverse index, and ids that churn.
- **C (links).** Would require a relation whose target is not an entity.
- **E (outside the graph).** Rejected for the machine-readable requirement;
  an in-memory overlay remains as the TUI's fast path in front of D.

## Open Questions Answered

- *Does the daemon snapshot need to be fresher than 5 s?* Not for the
  interactive case, where the overlay is fresher. Scripts that want the
  freshest value can read the store through the CLI (a story adds a
  `--fresh` read that skips the snapshot).
- *Where is a split pane's state?* Each pane's agent has its own entry. The
  mux shows counts over all of them. A session's own row shows its own
  state.
