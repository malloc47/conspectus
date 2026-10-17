# Mux capture, ghost muxes, and lightweight pins

Status: draft, pending operator review. No ADR, task, or code exists
for this yet; the open questions at the end gate the milestone and
tickets proposed after them.

## Summary

A session pin (ADR 0057) currently does four jobs at once:

1. **Mux lifecycle record.** It names a tmux session, its cwd, harness,
   and launch argv, and it relaunches that mux after it dies, resuming
   the last agent session (ADR 0058, ADR 0098).
2. **Unrealized intent.** An unlaunched pin is a placeholder session
   and mux row for work that hasn't started (ADR 0057, ADR 0094).
3. **Float to the top.** Pinned work leads the sessions and mux views.
4. **Naming.** The pin's `display_name` is the tmux name and the bound
   session's alias (ADR 0029 overlay).

Because job 1 is only available through a pin, the operator pins
nearly every mux, and job 3 stops meaning anything. Today's config has
15 pins and 15 live tmux sessions. Two of the pins run programs that
aren't agents (`emacsclient -c`, `conspectus serve`). Twelve carry the
same permission-bypass argv for their harness
(`claude --dangerously-skip-permissions`, or Codex's
`--dangerously-bypass-approvals-and-sandbox`), which is the only way
to relaunch with those flags today.

This plan splits the jobs apart:

- **Capture.** Conspectus records every mux it observes in a **mux
  ledger**, however the mux was created. Capturing replaces "pin
  create", "pin adopt", and the pin-binding sidecar. A captured mux
  keeps its name, cwd, launch argv, and last-hosted agent session
  after it dies.
- **Ghosts.** A captured mux that isn't running is a **ghost**. Ghosts
  look different from live muxes, show when they were last seen live,
  and relaunch (with resume) the way an unbound pin does today.
- **Reaping.** Ghosts not seen live for a long time can be cleaned up
  in bulk. Reaping only removes ledger records; it never touches tmux.
- **Pins.** A pin becomes a lightweight reference to a session, mux,
  or PR that keeps it at the top of its view and stops a pinned ghost
  from being reaped. It carries no launch spec, no mux name, and no
  binding.

## Vocabulary

| Term | Meaning |
|---|---|
| Captured mux | A mux with a ledger record. Every observed mux becomes one unless an ignore rule matches. |
| Live | The mux backend's latest successful probe lists the mux. |
| Ghost | Captured, and the latest successful probe of its backend and socket didn't list it. |
| Unknown | Captured, but its backend or socket wasn't probed successfully (tmux missing, probe error, socket not enumerated). Never shown as a ghost and never reaped. |
| Pin | User-authored reference to a node. Floats it to the top. Stops a pinned ghost from being reaped. |
| Reap | Bulk removal of old, unpinned ghost records. |
| Forget | Removal of one ghost record. |

How today's concepts map:

| Today | After |
|---|---|
| `pin create` + `pin launch` | `mux launch` (already exists, ADR 0096); the new mux is captured on sight |
| `pin adopt` | Nothing to do: every mux is captured |
| Unbound pin (mux gone, had run before) | Ghost mux |
| Unbound pin that never launched | See open question 8 |
| `PinStaleMux` (mux live, harness exited) | Live mux with "agent exited" status |
| `pin rebind` after external rename | Rename continuity in the ledger (see Identity) |
| `pin bind` (ambiguity override) | Existing declared `linked_to_mux` link (ADR 0014) |
| Pin-binding sidecar (ADR 0058) | Ledger `last_session` field |
| Pin-store registry (ADR 0090) | Removed: pins become user-scope only |
| `PinNode` (ADR 0084) | `pinned` attribute overlay on the target node |
| "Pins" group | "Pinned" group, holding only what the operator pinned |

## Data model

### The mux ledger

One file, `$XDG_STATE_HOME/conspectus/muxes.json` (with the usual
`~/.local/state` fallback), next to `pin-stores.json` and the TUI
state file. It is a **last-seen index**, which `docs/design.md`'s State
And Persistence section already lists as acceptable global state. It
is not `graph.bin`: `graph.bin` is thrown away on every format-version
bump (ADR 0082), and the ledger's history can't be rebuilt from disk
once a mux is gone.

```json
{
  "schema_version": 1,
  "muxes": [
    {
      "key": "tmux:conspectus-main",
      "backend": "tmux",
      "socket": null,
      "name": "conspectus-main",
      "first_seen": 1760000000,
      "last_seen_live": 1760600000,
      "incarnation": { "session_id": "$12", "created": 1760500000 },
      "cwd": "/home/me/src/conspectus",
      "launch": {
        "argv": ["claude", "--dangerously-skip-permissions"],
        "source": "conspectus"
      },
      "last_session": {
        "harness": "claude-code",
        "session_id": "e44d01cb-…",
        "observed": 1760600000
      },
      "origin": "observed"
    }
  ]
}
```

Fields:

- `key` is the graph's `MuxSessionId` native id (`tmux:<name>` or
  `tmux:<socket>:<name>`, ADR 0057). A ghost keeps the id its live mux
  had, so declared links and aliases keyed on the mux survive its
  death.
- `incarnation` tells one run of a name apart from the next. tmux
  supplies `#{session_id}` (`$N`, unique for the life of a server) and
  `#{session_created}`. Discovery reads `session_created` already;
  `session_id` is new.
- `launch` records how to relaunch. `source` is `conspectus` when a
  Conspectus launch wrote the argv, `observed` when it came from tmux's
  `#{pane_start_command}`, and `migrated` when it came from a pin. See
  open question 9.
- `last_session` is the ADR 0058 sidecar's content, kept per mux
  instead of per pin.
- `origin` is `observed`, `launched`, or `migrated`.

No payload is stored (ADR 0086 Tier 3). Launch argv is the one field
that needs care: a harness argv can carry a prompt.

### Identity and matching

When a probe lists a mux, the writer matches it to a record in this
order:

1. **Same incarnation**: same backend and socket, same
   `session_id` and `created`. This is the same tmux session, possibly
   renamed outside Conspectus, so the record's `name` and `key` follow
   it. External renames, which ADR 0057 recovers with `pin rebind`,
   heal on their own.
2. **Same key, record is a ghost**: the name came back, either from a
   relaunch or because someone recreated it by hand. The ghost comes
   back to life with a new incarnation. As in ADR 0102, `last_session`
   is kept only while that session shows activity after the new
   `created`.
3. **Otherwise** a new record is created.

tmux won't run two sessions with one name on one socket, so a live key
can't collide. Zellij has no session id, so it matches on rule 2 only.

Renames that Conspectus performs (ADR 0029 lockstep) rewrite the
record directly rather than waiting for rule 1.

### Graph projection

The ledger becomes a discovery input for the mux class:

- Live muxes keep their `MuxSession` nodes. The ledger adds
  `first_seen` and `last_seen_live` to them.
- Each ghost becomes a `MuxSession` node with
  `liveness = Ghost { last_seen_live }` (the field is new; live nodes
  carry `Live`, unprobed ones `Unknown`). One polymorphic node with a
  liveness attribute follows the same principle as the single `Fork`
  node in `CLAUDE.md`. A separate `GhostMux` kind was considered: it is
  safer by construction, but it doubles every mux code path and breaks
  id continuity across death.
- A ghost gets no live-only relations (`linked_to_mux`,
  `mux_contains_process`). It links to its last agent session through
  a new historical relation, `mux_last_hosted`, so "this session last
  ran in mux X" is in the graph without claiming it runs there now.
- A live mux's `last_session` also enters the resolver as a
  `Cached`-provenance `linked_to_mux` fallback. That gives every mux
  the stability that ADR 0102 point 3 gave pins (the previous binding
  is the weakest evidence), and it is the "Option A" that ADR 0058
  deferred.

**Every consumer that assumes a `MuxSession` is running must be
audited**: attach, rename, teardown (ADR 0093), mux chips (ADR 0072),
"float muxed sessions to top", header mux counts, the process-tree and
hook linkers, and mux-candidate counts on session rows. This audit is
the riskiest part of the change. The snapshot format version bumps.

### Pins

A pin is a reference to a graph node:

```toml
[pins]
schema_version = 2

[[pins.items]]
target = "mux:tmux:conspectus-main"
# optional; shown when the target is missing from the graph
label = "conspectus-main"
pinned_at = 2026-10-16T12:00:00Z
```

- Targets in v1 are agent sessions, muxes, and forge PRs. The encoding
  reuses `NodeId`'s stable string form, so other kinds can be added
  without a schema change.
- **Session pins follow lineage.** A pinned session that compacts or
  resumes (ADR 0018) keeps its pin on the lineage head.
- **PR pins outlive discovery.** A merged or closed PR can drop out of
  forge discovery. The pin's `label` then keeps a readable row, marked
  missing, until the operator unpins it.
- **Dangling pins** (any target that's missing) show in the Pinned
  group with a "missing" glyph and an unpin action, and raise a
  `PinDangling` diagnostic. They are never removed automatically.
- Pinned is an overlay attribute like an alias (ADR 0029), not a node.
  `PinNode`, `pin_targets_mux`, and `pin_realized_by_session`
  (ADR 0084) go away. Graph exports show `pinned: true` on the node.
- `schema_version = 2` stops older binaries from loading the new table
  (ADR 0057's unknown-version rule) rather than misreading it.

Where pins are stored is open question 6.

### Resolver changes

The pin binding pass (ADR 0057 §Resolver Binding Semantics,
ADR 0102) is removed. What it did that is still needed moves:

| Pin-pass job | New home |
|---|---|
| Bind a session to a pin's mux | The general `linked_to_mux` resolution, which already chooses one mux per session |
| Filter by harness | Not needed: a captured mux records whatever harness it hosts |
| Previous binding as weakest evidence | Ledger `last_session` as a `Cached` fallback for every live mux |
| One-to-one assignment | Already true per session; a mux with several sessions records the freshest as `last_session` |
| `PinStaleMux` | Mux status "agent exited", derived from a live mux with no attributed session plus `last_session` |
| `PinUnbound` (`last_session`) | Ghost node plus `mux_last_hosted` |
| `PinAmbiguous` | The existing ambiguous `linked_to_mux` slot (ADR 0077) |
| `PinDrift` | Dropped. The ledger records the cwd it observes, so there's no declared cwd to drift from |
| `PinDuplicate` | Can't happen: ledger keys are unique |

## Capture pipeline

### Who writes

- **The daemon**, at the end of each mux-class cycle.
- **In-process discovery** (TUI without a daemon, one-shot CLI cold
  rebuilds), at the same point where the resolved snapshot would be
  written to `graph.bin`.
- **Launch paths** (`mux new`, `mux launch`, relaunch, worktree
  stream), which write the record and its argv when they create the
  mux, without waiting for the next probe.

A client talking to a running daemon never runs discovery, so it never
writes the ledger. The daemon is the writer whenever it is up.

### Only fresh probes count

A writer contributes only the result of a mux probe it ran itself,
stamped with the probe time:

- A warm-start `graph.bin`, a daemon snapshot, or a memoized fragment
  from before this cycle (ADR 0099) never updates `last_seen_live`.
  Otherwise a TUI opened on an hour-old snapshot would mark dead muxes
  as live "now".
- Ghost status needs a **successful** probe of that backend and
  socket. If tmux is missing or `list-sessions` fails, the backend's
  muxes are Unknown, not ghosts. "No server running" counts as a
  successful, empty probe: that is a reboot, and every mux really is a
  ghost.
- The ledger also says which sockets matter. Probing
  `{default} ∪ {sockets in the ledger}` answers the question CSP-382
  left open (which non-default sockets to enumerate) without scanning
  all of `$TMUX_TMPDIR`. Until that lands, records on non-default
  sockets stay Unknown.

### Concurrency

Several processes can discover at once (TUI fallback, a CLI run, a
daemon starting up). Each writer:

1. takes an exclusive advisory lock on `muxes.lock` next to the ledger
   (`std::fs::File::lock`, stable since Rust 1.89; the toolchain is
   1.99, so no new dependency),
2. re-reads the ledger,
3. merges in its own fresh observations,
4. writes through the existing `write_atomic` temp-file-and-rename
   helper, and
5. releases the lock.

The merge is commutative. `first_seen` takes the minimum,
`last_seen_live` the maximum, `last_session` the one with the latest
`observed`, and a record is created if absent. Each writer re-reads
under the lock and contributes only what it observed, so the order of
writers doesn't change the result, and a reap can't be undone by a
writer holding an old copy. The only thing that brings a reaped record
back is a fresh probe showing the mux live, which is correct.

Electing a single discovery leader would also prevent duplicated
discovery work, but it doesn't make the ledger any more correct, and a
lease adds a liveness problem (a crashed leader). A discovery
singleflight lease is a separate, optional follow-up.

### Write cadence

`last_seen_live` is only needed to the precision of reaping thresholds
(days), so a live mux's record is rewritten at most every 10 minutes
unless something else changed (new mux, rename, `last_session`,
launch argv). When a writer sees a mux disappear, it stamps the time
of its own last live sighting of that mux, if it holds one in memory.
UI copy says "last seen live", which is honest about the gaps:
Conspectus only knows what it saw.

A mux created and killed between two observations is never captured.
With the daemon polling the mux class every few seconds that's rare,
and it's acceptable. A tmux `session-created` hook would close the gap,
but it means writing to the operator's tmux config, which is outside
the envelope. Deferred.

### Read-only modes

`--no-cache`, fixture and snapshot modes (ADRs 0067, 0068, 0069), the
dev scenario builder, and tests with no state home don't write the
ledger, the same way they skip `graph.bin`. Fixtures can carry a
ledger, so ghosts are snapshot-testable.

### Mutation envelope placement

The ledger is a category 2 write (rebuildable observation sidecar
under `$XDG_STATE_HOME/conspectus/`). `docs/capabilities.toml` already
allows `$STATE/conspectus/**` in the `read` context, so the manifest
only gains a comment citing the new ADR.

Prohibition 5 ("performs background mutation") needs a clarification
rather than a supersession. Its text forbids the daemon from writing
user-intent stores, renaming muxes, and spawning harnesses. The daemon
already writes `graph.bin`, so category 2 observation writes are
evidently allowed. ADR 0090 read the prohibition more strictly ("the
background daemon never registers stores"); the new ADR should state
the rule outright: the daemon may write category 2 observations, and
never category 1, 3, 4, or 5.

The question to settle is whether the ledger is observation at all.
Deleting it loses ghosts and relaunch argv, but no operator-authored
intent, provided the intent that matters (pins, launch customization,
ignore rules) lives in TOML. Open questions 3 and 9 settle that line.

## Ghost lifecycle

```
            probe lists it              probe (ok) omits it
(new) ──────────────────────> Live ─────────────────────────> Ghost
                               ▲  │                             │ │
                               │  │ probe fails / socket        │ │ relaunch or
                               │  ▼ not enumerated              │ │ recreated by name
                               │ Unknown                        │ │
                               └────────────────────────────────┘ │
                                                                  │ reap / forget
                                                                  ▼
                                                            (record removed)
```

### Relaunch

`Enter` on a ghost, or `conspectus mux relaunch <name>`, runs the
ADR 0057 / ADR 0058 / ADR 0098 / ADR 0103 launch logic with the ledger
in place of the pin:

- name and socket come from the record, and cwd from `cwd` (worktree
  case below),
- argv precedence: recorded `launch.argv`, then the launch profile
  (project, then user) if open question 9 adds one, then
  `HarnessAdapter::launch_argv`,
- resume: `last_session` is followed forward along lineage and spliced
  in with `resume_argv` after the harness binary (ADR 0098). A failed
  resume falls back to a fresh launch (ADR 0103), as today,
- a ghost with no harness (a bare shell mux, or `conspectus serve`)
  relaunches with its recorded argv, or with a shell when none was
  recorded.

The "live mux, agent exited" relaunch keeps its ADR 0102 rules: attach
when pane-process evidence shows the harness running, `send-keys` the
Conspectus-built argv only when it doesn't (ADR 0028, ADR 0087
category 3).

Orchestrator-owned ghosts (agent-deck, ADR 0060) are captured and
shown, but relaunch is disabled with "owned by agent-deck", because
the orchestrator owns their lifecycle (see open question 19).

### Worktrees

Worktree-backed pins (ADR 0094) declared a branch to realize at
launch. With pins gone, the worktree stream is `mux launch` with the
worktree toggle on, which ADR 0096 already provides. The captured
record stores the realized worktree path as its cwd. A ghost whose cwd
no longer exists can't relaunch: the row says so and offers "forget".
Worktree close-down (`worktree close`), which today drops the pins
declared in the worktree, instead forgets the ghosts whose cwd is
inside it.

### Teardown

Killing a mux from Conspectus (ADR 0093) leaves a ghost, which is the
point: stop now, resume later. The mux action menu gains "kill and
forget" for when the operator means "done with this".

### Reaping

- `conspectus mux reap [--older-than <dur>] [--dry-run]` removes
  ghosts whose `last_seen_live` is older than the threshold
  (`[mux.ghosts] reap_after`, default in open question 11).
- `conspectus mux forget <mux>` removes one ghost. It refuses a live
  mux.
- In the TUI, a "Clean up ghosts" entry in the mux action menu opens a
  modal listing the candidates with their ages. The operator can
  deselect any of them before confirming (menu-first, per the TUI
  discoverability preference).
- Pinned ghosts and Unknown records are never candidates. A
  ghost whose last session is pinned can be reaped; the session row
  remains.
- Reaping removes ledger records only. It never touches tmux,
  harness state, or transcripts.

### Zellij

Zellij keeps its own ghosts: `list-sessions` reports `EXITED - attach
to resurrect`, and discovery already parses that flag. An exited zellij
session becomes a ghost, and relaunching it runs `zellij attach`, which
resurrects it with zellij's layout. When zellij has dropped the session
itself, relaunch falls back to creating it. This depends on CSP-618
(zellij pins launch through tmux today).

## Surfaces

### CLI

| Command | Change |
|---|---|
| `mux list [--ghosts \| --live \| --all]` | New, or an extension of the existing mux table, with liveness and last-seen columns |
| `mux relaunch <name>` | New. Ghost relaunch, or agent-exited relaunch on a live mux |
| `mux forget <name>`, `mux reap` | New |
| `mux new`, `mux launch` | Unchanged, plus a ledger write with argv at launch |
| `pin add <target>`, `pin rm <target>`, `pin list` | New lightweight pins. Targets accept a mux name, session id or prefix, or PR reference (`owner/repo#123`) |
| `pin create`, `launch`, `attach`, `bind`, `rebind`, `adopt`, `rename`, `show` | Retired. Each prints the replacement (`mux launch`, `mux relaunch`, `declared create`, …); see open question 15 |
| `pin migrate [--dry-run]` | New, one-time. See Migration |

### TUI

- **Pinned group** at the top of the sessions, mux, and PRs views,
  holding only what's pinned. In the session-first sessions view, a
  pinned mux contributes its current (or last) session row, which
  keeps today's behavior for pinned muxes.
- **Pin toggle** as a row action on session, mux, and PR rows, in the
  action menus first, with a direct key as an accelerator. The Pins
  modal (`p`) and the `N` / `B` / `b` / `A` pin shortcuts go away; `N`
  becomes "mux launch with worktree" (ADR 0096). This touches CSP-619
  (bringing the Pins overlay onto the shared contract), which becomes
  partly moot.
- **Mux view**: ghosts sort below live muxes inside each group, dimmed,
  with their own glyph (picked against ADR 0073's kind identity) and a
  "seen 3d ago" age. A Controls toggle (ADR 0031) shows or hides
  ghosts. The header shows `11 mux · 4 ghost`.
- **Sessions view**: no placeholder rows. The historical session that
  a ghost last hosted already has a row (its transcript is on disk);
  it gains a "was in `conspectus-main`" chip, and `Enter` relaunches
  the ghost with resume. This removes the placeholder-session-row
  machinery in `src/tui/rows/sessions.rs`.
- **Detail pane**: a mux shows liveness, first seen, last seen live,
  recorded launch argv, and last session. A ghost's Related list
  includes `mux_last_hosted`.

### Display names

Today a pin's `display_name` overrides the bound session's title. With
pins gone, sessions that relied on it would fall back to the harness
title. Many Claude Code sessions have empty titles (31 in the dev tree,
per ADR 0057). Proposed precedence: `alias > title > mux name >
id-suffix`, where the mux name comes from the live mux or the ghost the
session last ran in. See open question 10.

## Migration

`conspectus pin migrate` is operator-run (a TOML write is category 1,
which the daemon may not do). It:

1. reads every `[[pins.entries]]` (project stores through the ADR 0090
   registry, plus the user store),
2. writes a ledger record per pin: `key` from `mux.native_id()`, cwd,
   `launch.argv` with `source = "migrated"`, `last_session` from the
   pin's ADR 0058 sidecar, and `last_seen_live` from the sidecar's
   `observed_epoch` (or now, when the mux is live),
3. writes lightweight pins per open question 16,
4. removes the `[[pins.entries]]` tables it converted, keeping
   comments and formatting (`toml_edit`), and
5. prints what it did. `--dry-run` prints the plan only.

Until the operator migrates, discovery still reads the old schema and
treats each `[[pins.entries]]` as an in-memory ledger seed, so nothing
disappears on upgrade. A `LegacyPins` diagnostic suggests running
`pin migrate`. After one release, legacy loading is removed along with
the pin-binding sidecar directory and `pin-stores.json`.

## Implications

### ADRs

- **New ADR: mux capture ledger and ghost muxes.** Supersedes ADR 0058
  (sidecar) and ADR 0102 (binding pass), and supersedes in part ADR
  0057 (binding, launch, `adopt` / `rebind`) and ADR 0094 (pin-backed
  worktree streams). Clarifies ADR 0087 prohibition 5 and amends ADR
  0090's reading of it.
- **New ADR: lightweight pins.** Supersedes in part ADR 0057 (schema,
  CLI, TUI) and fully supersedes ADR 0084 and ADR 0090.
- ADR 0087's Related ADRs lists both. ADR 0096 and ADR 0095 drop "can
  later be adopted as a pin". ADR 0093 notes that teardown leaves a
  ghost.
- `docs/design.md`: rewrite Session Pins and Mux Lifecycle; add the
  ledger to State And Persistence; update Decisions.
- `docs/operations.md` (Session pins), `docs/pins-walkthrough.md`, the
  README, and the showcase scenario (ADR 0070) are rewritten.

### Code (by area)

- `src/pins.rs`, `src/discovery/pins.rs`, `src/resolve/pins.rs`,
  `src/pin_bindings.rs`, `src/pin_store_registry.rs`: the old pin model
  is replaced. The new pin store and overlay are much smaller.
- `src/discovery/tmux`: `#{session_id}` added to `TMUX_LIST_FORMAT`;
  per-socket probe status is reported so liveness can be Unknown.
- New ledger module: schema, lock, merge, matching, cadence.
- `src/model`: `MuxSessionNode.liveness`, `first_seen`,
  `last_seen_live`; `RelationKind::MuxLastHosted`; `PinNode` and the
  pin relation kinds and diagnostics removed; snapshot version bump.
- `src/server`: a ledger write at the end of the mux-class cycle.
- `src/cli/pin.rs` (1,455 lines): mostly removed; launch logic moves
  behind `mux relaunch`.
- `src/tui/rows/{sessions,mux,prs}.rs`, `src/tui/widgets/pins*`,
  keymap, controls: Pinned group, ghost rows, pin toggle; the Pins
  modal and the placeholder rows are removed.
- Tests: `src/resolve/pins_tests.rs`, `src/tui/widgets/pins_tests.rs`,
  and the pin parts of the sessions and mux row tests are rewritten.
  New tests cover sparse and ambiguous cases: name reuse, external
  rename, probe failure, unenumerated socket, concurrent writers, a
  reap racing a writer, and a dangling pin.

### Existing backlog

| Task | Effect |
|---|---|
| CSP-617 (`pin adopt` misses live tmux) | Obsolete; close when adopt is retired |
| CSP-618 (zellij pins via tmux) | Becomes the zellij ghost-relaunch ticket's blocker, or folds into it |
| CSP-382 (non-default socket discovery) | Unblocked in part: the ledger supplies the socket set |
| CSP-385 (glob pin patterns) | Obsolete; launch profiles cover the useful part |
| CSP-455, CSP-455.01, CSP-434 (pin create form cwd picker) | Retarget to the mux launch form |
| CSP-619 (Controls / Pins overlay contract) | Pins half goes away |
| CSP-170 (resume an un-muxed session into a mux) | Related: relaunch-with-resume is the same mechanism with a new mux name |
| m-20 (0.1.0 release) | Sequencing; see open question 15 |

### Risks

- **Ghosts leaking into live semantics** (attaching a ghost, counting
  it as a mux, floating its session as "muxed"). The consumer audit and
  liveness-aware helpers on `SnapshotIndex` reduce this; snapshot tests
  with ghosts in fixtures catch it.
- **False ghosts** from a stale snapshot or a failed probe. The
  fresh-probe-only rule and the Unknown state prevent them, and
  reaping never touches Unknown.
- **Ledger loss** (deleted state dir, new machine) loses ghosts and
  recorded argv, but pins and TOML launch profiles survive. That's the
  point of keeping intent out of the ledger.
- **Noise**: capturing everything fills the mux view with ghosts of
  throwaway shells. Ignore rules, the ghost toggle, and reaping keep it
  in check, and open question 1 has a softer option.

## Open questions

Each has a recommendation. The ones marked **blocking** change ticket
scope; the rest proceed as recommended unless you object.

1. **What gets captured?** Every mux, or only muxes that hosted an
   agent session at some point? *Recommend: every mux*, as you asked
   (the `conspectus-serve` pin shows non-agent muxes matter), with
   `[mux.capture] ignore = ["<glob>", …]` rules. Ghosts that never
   hosted an agent could take a shorter reap threshold.
2. **Identity across rename and recreation.** *Recommend the matching
   order above* (same incarnation, then a ghost with the same key,
   then a new record). The alternative, a name-only key, loses external
   renames; an incarnation-only key turns every relaunch into a new
   record.
3. **Is the ledger observation (category 2) or intent (category 1)?**
   *Recommend category 2* in `$XDG_STATE_HOME`, with all intent (pins,
   launch customization, ignore rules) in TOML. That requires the
   prohibition 5 clarification.
4. **Coordinating writers.** *Recommend merge-under-lock with
   `std::fs::File::lock`* and a commutative merge, not leader election.
   A discovery singleflight lease is a later optimization.
5. **How precise is "last live"?** *Recommend a 10-minute resolution
   while live, plus a stamp when the mux disappears, shown as "last seen
   live".*
6. **Blocking. Where do pins live?** *Recommend user scope only*: pins
   are personal "keep this handy" state, not project-rooted
   relationships, which also retires the pin-store registry. Sub-question:
   the existing user store is `$XDG_CONFIG_HOME/conspectus/config.toml`.
   If that file is managed by home-manager (a read-only store symlink),
   pin writes fail. Write pins into `config.toml` like global aliases,
   or into a dedicated `$XDG_CONFIG_HOME/conspectus/pins.toml`, which
   needs a one-line ADR 0087 category 1 extension?
7. **Pin targets and identity.** *Recommend sessions, muxes, and PRs in
   v1*, with session pins following lineage, PR pins keeping a label
   after the PR leaves discovery, and dangling pins kept until unpinned.
   Pinning a session doesn't protect the ghost it last ran in.
8. **Blocking. Keep "declare without launching"?** An unlaunched pin
   is a placeholder for work not yet started (and the only way to stage
   a worktree stream without starting an agent). *Recommend dropping
   it.* Creating means launching, detached if wanted, and migration
   turns unlaunched pins into ghost records (`origin = "migrated"`) so
   none are lost. Keeping it means a `mux plan` command that writes a
   never-live ledger record (`origin = "declared"`), which is cheap but
   brings intent back into the ledger.
9. **Blocking. Where does launch customization live?** Twelve of the
   15 current pins carry the same bypass flag for their harness in two
   different repos, which makes it a per-harness preference rather than
   a per-mux one. *Recommend both of these*:
   - The ledger records the argv of every Conspectus launch, so a ghost
     relaunches exactly as it was started.
   - A launch profile (`[launch.claude-code] argv = [...]`) in the user
     config, which a project's `.conspectus.toml` can override. It is
     the TOML intent for "how to start this harness", the default for
     `mux launch`, and the argv for ghosts with no recorded argv.

   For muxes created outside Conspectus, should the ledger record
   tmux's `pane_start_command`? It can hold a prompt
   (`claude "fix the bug"`), which ADR 0086 keeps out of Tier 3.
   *Recommend recording it only after a harness-adapter sanitizer
   strips positional prompt arguments*, and otherwise falling back to
   the profile.
10. **Blocking. Session names without pins.** *Recommend
    `alias > title > mux name > id-suffix`*, so untitled sessions show
    the mux name. Migration can optionally write explicit aliases for
    sessions currently bound to pins whose `display_name` differs from
    the session title.
11. **Blocking. Reaping policy.** *Recommend operator-initiated only*
    (CLI plus the TUI modal) with a default threshold of 14 days and no
    automatic deletion in v1. Ghosts stay visible in the mux view until
    reaped, behind the ghost toggle. Alternatives: automatic reaping
    by the daemon (allowed as category 2, but it deletes things the
    operator may want), or auto-hiding ghosts past a horizon without
    deleting them.
12. **Ghost visibility.** *Recommend*: shown in the mux view by default
    (toggle in Controls); in the sessions view, shown as a chip on the
    historical session rather than as a row.
13. **Retire the pin binding pass.** *Recommend yes*, with ADR 0102's
    previous-binding stickiness applied to every captured mux through
    the ledger's `Cached` fallback.
14. **Drop `PinNode`.** *Recommend yes*, in favor of a `pinned`
    attribute; dangling pins surface as diagnostics and rows.
15. **Blocking. Compatibility and sequencing against 0.1.0.**
    *Recommend landing before 0.1.0 and breaking the `pin` CLI
    cleanly*: retired verbs print their replacement, there are no
    shims, and legacy TOML is read for one release. Shipping 0.1.0 with
    the heavy pin model and then breaking it publicly costs more. This
    adds a milestone to the release's critical path. Alternatively,
    release first and land this as 0.2.0.
16. **Blocking. What does migration pin?** *Recommend capturing every
    pin and pinning none by default*, with `--pin <id>…` / `--pin-all`
    flags and a printed list. You said you pin practically everything,
    so carrying all 15 over as pins would recreate the problem. Nothing
    is lost: reaping is manual and dry-run first.
17. **Unprobed sockets.** *Recommend the Unknown state, with the
    ledger's sockets added to the probe set* (absorbing part of
    CSP-382).
18. **Zellij.** *Recommend treating `EXITED` as a ghost and
    relaunching through `zellij attach`*, after CSP-618.
19. **Orchestrator-owned muxes** (agent-deck). *Recommend capturing
    and showing them, with relaunch disabled.* Their lifecycle belongs
    to the orchestrator (ADR 0060, CSP-451).
20. **Bulk relaunch after reboot** ("bring back all pinned ghosts").
    *Recommend a follow-up ticket, not v1.* It is the agent-deck
    "restore my deck" flow and fits pins naturally.

## Proposed milestone and tickets

Milestone: **Mux Capture And Lightweight Pins**. All tasks get a
`mux-capture` label. These are created after the blocking questions
are answered; T-numbers become CSP ids then.

| # | Ticket | Depends on |
|---|---|---|
| T1 | ADR: mux capture ledger and ghost muxes. Supersessions, ADR 0087 prohibition 5 clarification | — |
| T2 | ADR: lightweight pins. Supersessions; `docs/design.md` Session Pins and Mux Lifecycle rewrite | — |
| T3 | Ledger store: schema, lock, merge, identity matching, cadence, read-only modes; multi-process tests | T1 |
| T4 | tmux discovery: `#{session_id}`, per-socket probe status, ledger sockets in the probe set | T1 |
| T5 | Capture pass: daemon and in-process writers, fresh-probe gating, `last_session` recording (replaces the ADR 0058 sidecar writer), launch-time records with argv | T3, T4 |
| T6 | Graph projection: `liveness`, ghost nodes, `mux_last_hosted`, `Cached` fallback, snapshot bump, live-assumption audit | T5 |
| T7 | Relaunch: `mux relaunch`, ghost `Enter`, argv precedence, resume splice, agent-exited path moved from pin launch, worktree-missing handling | T6 |
| T8 | Lightweight pin store and overlay: schema v2, targets, lineage-following, dangling pins, `PinNode` removal | T2, T6 |
| T9 | Pin CLI: `pin add` / `rm` / `list`; retire old verbs with replacement messages | T8 |
| T10 | TUI: Pinned group in sessions, mux, and PRs views; pin toggle; ghost rows and chips; Controls ghost toggle; Pins modal removal; `--snapshot` coverage | T7, T8 |
| T11 | Reaping: `mux reap` / `forget`, TUI clean-up modal, `[mux.ghosts]` config, pinned protection, worktree close-down forgets ghosts | T6, T8 |
| T12 | Capture ignore rules (`[mux.capture] ignore`) | T5 |
| T13 | Migration: `pin migrate`, legacy in-memory seeding, `LegacyPins` diagnostic, sidecar import | T5, T8 |
| T14 | Launch profiles (`[launch.<harness>]`, user scope with project override), if question 9 keeps them | T7 |
| T15 | Session display-name fallback to the mux name, if question 10 keeps it | T6 |
| T16 | Zellij ghosts: `EXITED` as ghost, resurrect through `zellij attach` | T7, CSP-618 |
| T17 | Docs and fixtures: operations guide, pins walkthrough, README, showcase and dev scenarios with ghosts | T10, T11, T13 |
| T18 | Follow-up: bulk relaunch of pinned ghosts | T10 |
| T19 | Retire legacy pin loading, sidecar directory, and `pin-stores.json` (one release after T13) | T13 |

Suggested order: T1 and T2 first (the decisions), then the capture
spine T3 → T4 → T5 → T6. With the spine in place, T7, T8, and T11 can
proceed in parallel. T10 and T13 come before any release that includes
the change. T18 and T19 come later.
