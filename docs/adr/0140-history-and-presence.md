# ADR 0140: History And Presence

## Status

Accepted. Supersedes ADR 0058 (the pin-binding sidecar) and ADR 0102
(the pin binding pass and its stickiness). Amends ADR 0057 (its
resolver binding semantics are removed) and ADR 0086 (Tier 3 records
may carry a mux's launch argv verbatim). Part of the history, rebuild,
and pin slots plan with ADRs 0141, 0142, 0143, and 0144.

## Context

Session pins (ADR 0057) are the only way to relaunch a mux after it
dies with its name, launch command, and last session. The pin entry
holds the declaration, the resolver binds it to a live session every
cycle (ADR 0057, ADR 0102), and a sidecar remembers the last binding
so a relaunch can resume it (ADR 0058). So the operator pins nearly
every mux, 15 of them on the operator's machine, most with a custom
launch argv, and "pinned" no longer singles anything out. A mux
started by hand is not rebuildable at all, a worktree that was removed
leaves no trace, and the binding pass carries its own copy of the
resolver's session-to-mux logic.

The plan in `docs/plans/history-rebuild-and-pin-slots.md`, reviewed by
the operator on 2026-10-19 and 2026-10-21, splits that apart. A
history layer records everything rebuildable Conspectus observes,
pinned or not. Pins become editable slots on top of it (ADR 0141), and
rebuilds read from either (ADR 0142). This ADR records the history
half: what is recorded, where, how a gone entity is told apart from
one that just wasn't scanned, how concurrent writers agree, and how
history enters the graph.

## Decision

### Three layers

| Layer | Holds | Written by | Where |
|---|---|---|---|
| Graph | What exists now | Discovery, every cycle | Memory and `graph.bin` (rebuildable) |
| History | What Conspectus has observed, gone entities included, with what it takes to rebuild them | Discovery and launch paths, automatically | `$XDG_STATE_HOME/conspectus/history/` |
| Pins | Ordered slots the operator keeps at hand | The operator only | `$XDG_DATA_HOME/conspectus/pins.toml` (ADR 0141) |

History is the "last-seen index" `docs/design.md` already allows as
global state, and a category 2 observation under the envelope (ADR
0143). It is not kept in `graph.bin`, because `graph.bin` is thrown
away on every format bump (ADR 0082) and a gone entity can't be
rediscovered.

### Store

```
$XDG_STATE_HOME/conspectus/history/
  history.lock
  muxes.json
  checkouts.json
  repos.json
  prs.json
```

Each file is `{ "schema_version": 1, "records": [...] }`. A file with
a newer `schema_version` is read for nothing and left untouched, so an
older binary doesn't clobber a newer one's records. Unknown fields,
in the file or in a record, survive a rewrite.

Every record has a common envelope and a recipe for its kind:

```json
{
  "id": "h_7f3a9c0e21d4",
  "key": "mux_session:tmux:conspectus-main",
  "first_seen": 1760000000,
  "last_seen": 1760600000,
  "origin": "observed",
  "display": { "label": "conspectus-main", "context": "claude · ~/src/conspectus" },
  "rebuilt_from": null,
  "recipe": { "...": "per kind" }
}
```

- `id` is a stable random id (`h_` and 12 hex digits). Pin slots point
  at records by id (ADR 0141), so a record can change its key, after an
  outside rename or a moved worktree, without `pins.toml` being
  rewritten. The daemon can't write `pins.toml` (ADR 0143).
- `key` is the graph `NodeId` the record last matched, in its display
  form. Projection rebuilds the `NodeId` from the recipe, not by
  parsing the key.
- `origin` is `observed` (discovery saw it), `launched` (Conspectus
  created it), or `migrated` (`pin migrate` seeded it).
- `display` is what the History and Pinned views show once the entity
  is gone.
- `rebuilt_from` names the record a rebuild replaced, when the rebuilt
  entity landed under a different key (a worktree at a new path).

| Kind | Recipe |
|---|---|
| Mux | `backend`, `socket`, `name`, `cwd`, `incarnation` (`session_id`, `created`), `argv` and `argv_source` (`conspectus` or `observed`), `harness`, `orchestrator`, `hosted` (harness, session id, first and last seen; the 10 most recent) |
| Checkout | `repo` (the repo record's key), `root`, `linked`, `branch`, `head`, `upstream` |
| Repo | `common_dir`, `remotes` (name and fetch URL), `primary_root`, `default_branch` |
| PR | `provider`, `host`, `owner`, `repo`, `number`, `title`, `state`, `draft`, `url`, `head_ref` |

A mux's launch argv is recorded verbatim. For a mux Conspectus started
it is the argv Conspectus built (`argv_source = "conspectus"`); for any
other mux it is tmux's `pane_start_command` (`observed`), when tmux
reports one. A harness argv can carry a prompt. Conspectus already
reads comparably sensitive state, and a scrubber would have to guess
which tokens are prompts, so this ADR amends ADR 0086 Tier 3 to allow
the recorded argv rather than adding a sanitizer. Nothing else in a
record is payload.

Branches have no records of their own. A branch's last commit lives
in the checkout record that had it checked out.

### Presence

The graph gains a sparse `node_presence` map next to
`node_provenance`:

- **Live**: discovered this cycle. The default, so a live node has no
  entry.
- **Gone** `{ last_seen }`: a targeted check of the recorded entity
  failed.
- **Unknown** `{ last_seen }`: the check couldn't run.

**Absence from a scan never means gone.** Discovery's reach depends on
scan roots, on which mux sockets were probed, and on whether the forge
was queried this cycle. A TUI started under another scan root must not
mark a repo's worktrees gone. Gone takes a targeted negative check of
the record itself:

| Kind | Live | Gone | Unknown |
|---|---|---|---|
| Mux | A successful probe of its backend and socket listed it | That probe succeeded and didn't list it. "No server running" counts as success: after a reboot every mux really is gone | The backend binary is missing, the probe failed, or the socket wasn't probed |
| Checkout | Its root exists | Its root is missing and the root's parent exists | The parent is missing too (an unmounted drive), or a permission error |
| Repo | Its common dir exists | Its common dir is missing and the parent exists | As for checkouts |
| PR | Returned by the latest listing | Never gone; the state comes from the last listing that returned it | — |

The checks are a `stat` per path record and the mux probes discovery
already runs, so they run every cycle for every record. Mux probes
report a status per backend and socket: listed, empty (no server),
failed, or unavailable. The probed socket set is the default socket
plus every socket a mux record or pin slot names. A zellij session
zellij lists as `EXITED` is gone (zellij keeps it to resurrect).

A checkout whose root exists but holds another repo's checkout is a
new record under its new key, and the old record goes gone on its own
check.

### Identity and matching

When a probe lists a mux, the writer matches it to a record in this
order:

1. **Same incarnation**: backend, socket, tmux's `#{session_id}`, and
   `#{session_created}`. It is the same session, possibly renamed
   outside Conspectus, so the record's name and key follow it. This
   heals what `pin rebind` fixed by hand.
2. **Same key, different incarnation**: the name came back (a rebuild,
   or recreated by hand). The record comes back to life with the new
   incarnation. Hosted sessions with no activity after the new
   incarnation's `created` don't carry over (ADR 0102's rule).
3. **Otherwise** a new record.

Within one merge, every incarnation match is made before any key
match, so a rename and a new session taking the old name land on the
right records. Zellij has no session id and matches on key only.

Checkouts match on the checkout id (repo and root), repos on the
common dir, and PRs on the PR id. A worktree rebuilt at another path
is a new record whose `rebuilt_from` names the old one (ADR 0142).

### Writers and merge

- **The daemon** writes at the end of a cycle, for the provider
  classes that cycle probed fresh: mux records after a mux probe,
  checkout and repo records after a git probe, PR records after a
  forge listing. A hook-ingest re-resolve probes nothing and writes
  nothing.
- **In-process discovery** (the TUI or a CLI command with no daemon)
  writes where it would write `graph.bin`.
- **Rebuild and launch paths** (`mux new`, `mux launch`, rebuild) write
  the record, with the argv they used and `origin = "launched"`, when
  they create the entity, without waiting for the next probe.
- **A daemon client** doesn't discover, so it doesn't write.

Each writer takes an exclusive advisory lock on `history.lock`
(`std::fs::File::lock`, so no new dependency), re-reads the files,
merges in only the observations it made itself, and writes each
changed file through the shared atomic write. The merge is
commutative: `first_seen` takes the minimum, `last_seen` the maximum,
hosted-session lists are unioned and truncated, mutable recipe fields
(a mux's name and cwd, a checkout's branch and head) come from the
newest sighting, and missing records are created. So any number of
processes can write in any order. A writer holding an old copy can't
undo a reap, because it merges observations, not copies, and only a
fresh sighting brings a reaped record back, which is correct.

Only fresh probes count. A warm-start `graph.bin`, a daemon snapshot,
or a discovery cache entry from an earlier cycle (ADR 0099) never
moves `last_seen`. A live record's `last_seen` is rewritten at most
every 10 minutes unless something else in it changed; reap thresholds
are in days, so that precision is enough, and quiet cycles leave the
files alone. A mux created and killed between two probes is never
recorded; catching it would take a tmux hook in the operator's
configuration, which is outside the envelope.

`--no-cache`, the fixture and snapshot modes (ADRs 0067, 0068, 0069),
the dev scenario builder, and runs with no state home never write
history. They still read it, except fixture modes, which take history
from the fixture so gone entities are snapshot-testable.

### Projection into the graph

After discovery, the history provider loads the records, runs the
presence checks, and adds to the snapshot:

- `history`: the records themselves, so views, slots, and exports have
  recipes and record ids without reading the store, and fixtures can
  carry them.
- A node for every record that isn't live this cycle, built from its
  recipe, with its presence. A ghost mux is a `MuxSessionNode` named
  after the record. A gone checkout or repo is the node its key names.
  PRs stay in the graph only while listed; a PR record that left the
  listing projects a `ForgePrNode` only when a pin slot targets it.
- `mux_last_hosted`: from a ghost mux to the most recent session it
  hosted, a `Cached` candidate. A ghost has no live-only relations: no
  `linked_to_mux`, no `mux_contains_process`, no `rooted_in`.
- `rebuilt_from`: from a rebuilt checkout to the one it replaced.
- A `Cached` `linked_to_mux` fallback from each live mux's most recent
  hosted session, when that session still exists and was active after
  the mux's `created`. It loses to any current evidence, so it only
  holds an attribution when evidence drifts. This is ADR 0102's
  stickiness, for every mux instead of pinned ones, and the "Option A"
  ADR 0058 deferred.

Code that acts on nodes asks a presence-aware helper on
`SnapshotIndex` before treating one as live. Attach, rename, teardown,
mux chips, header counts, "float muxed to top", the process-tree and
hook linkers, worktree actions, cwd-derived git probes, and Reveal all
skip or refuse gone nodes.

### Ignore rules and reaping

- `[history.ignore]` in the user config takes globs per kind
  (`mux = ["scratch*"]`, matched on the mux name; `checkout` and
  `repo` on the path; `pr` on `owner/repo#number`). Matching entities
  are never recorded.
- `conspectus history reap [--kind <k>] [--older-than <dur>]
  [--dry-run]` removes gone records past a per-kind threshold from
  `[history.reap_after]`: muxes after 14 days, checkouts after 30,
  PRs 30 days after they left the listing, and repos only through
  `history forget <ref>`, which removes one record.
- A record a pin slot targets is never reaped, and neither is an
  Unknown one.
- Reaping is operator-initiated only. It removes history records and
  never touches tmux, git, or the filesystem.
- `worktree close`, which used to drop the pins declared in the
  worktree, forgets the gone records rooted in it instead.

### What goes away

- The pin binding pass (ADR 0057, ADR 0102), its synthesized
  `linked_to_mux` links, `pin_realized_by_session`, and the
  `PinUnbound`, `PinStaleMux`, `PinAmbiguous`, and `PinDrift`
  diagnostics. A slot's target is an explicit reference (ADR 0141), so
  nothing needs inferring, and session-to-mux attribution stays with
  the general resolver.
- The pin-binding sidecar under `$XDG_CACHE_HOME/conspectus/pin-bindings/`
  (ADR 0058). A mux record's hosted sessions replace it. The legacy
  loader (ADR 0141) reads it once, for `pin migrate`.

## Consequences

- Any mux, worktree, or repo Conspectus has seen can be rebuilt
  without having been pinned, and pins no longer have to carry a
  mux's lifecycle.
- An outside rename heals itself, and a mux recreated under its old
  name gets its record back.
- Gone nodes are in the graph, so the detail pane, search, Reveal, and
  exports work on them. Code that assumed every node exists now has to
  check presence; the live-assumption audit is part of the projection
  work.
- `MuxSessionNode` gains the backend's own session id; the tmux list
  format gains `#{session_id}`. `CheckoutNode` gains its head commit.
  The snapshot format version moves to 8.
- Deleting the history directory loses gone entities and their
  recorded commands, and no operator intent.
- History grows with every mux the operator creates until it is
  reaped. Records are small, and reaping is one command.

## Alternatives Considered

- **Keep history in `graph.bin`.** One file fewer, but `graph.bin` is
  rebuilt cold on every format bump, which would erase every gone
  entity, and it is a cache by charter (ADR 0082).
- **Identify muxes by name only.** Simpler, but an outside rename
  would look like a death plus a new mux, and a name reused for
  unrelated work would inherit the old record's sessions.
- **One elected writer** (the daemon when it runs, else the TUI).
  Avoids a lock, but election needs liveness checks, a daemonless CLI
  run would record nothing, and handing over between writers is where
  observations get lost. A lock around a commutative merge has neither
  problem.
- **Treat absence from a scan as gone.** Correct only when every scan
  sees everything; any narrower scan root would mark live entities
  gone.
- **Automatic reaping.** Reaping removes the only copy of a gone
  entity's recipe. The operator chose manual reaping with thresholds.
- **Scrub prompts from recorded argv.** A scrubber can't tell a prompt
  from a flag value in an arbitrary wrapper argv, and a half-scrubbed
  argv rebuilds the wrong command. The operator chose verbatim argv.
- **SQLite for history.** ADR 0082 retired SQLite persistence. A few
  JSON files hold hundreds of small records comfortably.

## Open Questions Answered

- *Does a failed probe mark muxes gone?* No. Only a successful probe
  that doesn't list a mux does; a failure leaves its muxes Unknown.
- *What happens after a reboot?* The tmux server isn't running, the
  probe succeeds with no sessions, and every recorded mux is gone,
  ready to rebuild.
- *Can two processes corrupt history?* No. Writes are serialized by the
  lock, each write is atomic, and the merge doesn't depend on order.
- *Is a launched mux recorded before discovery sees it?* Yes, by the
  launch path, with the argv it used.

## Related ADRs

- ADR 0018 — session lineage; a session slot follows its head (ADR
  0141).
- ADR 0057 — session pins; binding removed by this ADR.
- ADR 0058 — pin session continuity; superseded.
- ADR 0082, ADR 0083 — `graph.bin` as a rebuildable cache.
- ADR 0086 — payload tiers; Tier 3 amended for recorded argv.
- ADR 0099 — caller-owned discovery caches; cached results never move
  `last_seen`.
- ADR 0102 — one-to-one pin bindings; superseded, its stickiness moved
  to every mux.
- ADR 0141 — pin slots.
- ADR 0142 — rebuild plans.
- ADR 0143 — the mutation envelope; history is category 2.
- ADR 0144 — the Pinned and History views.
