# History, rebuild, and pin slots

Status: accepted plan (operator review 2026-10-19 and 2026-10-21), now
carried by ADRs 0140 (history and presence), 0141 (pin slots), 0142
(rebuild plans and executors), 0143 (the mutation envelope), and 0144
(the Pinned and History views). The ADRs, not this document, are
authoritative. Milestone m-23 (CSP-656..CSP-691) tracks the work.

The first revision (2026-10-16) proposed a mux-only ledger and pins as
lightweight references. Operator review on 2026-10-19 widened it:

- Anything in the graph that could conceivably be rebuilt can be
  pinned. Only ephemeral entities (runtime processes) can't.
- Observed history stays separate from pinning. Any observed mux (with
  its command and session) or worktree can be rebuilt without having
  been pinned.
- Pinned entities get their own **Pinned** view, a peer of Sessions,
  Mux, PRs, and the others, holding a list of mixed kinds.
- A pin stays an editable **slot**, as pins are today: the operator can
  rename it, change its launch command, or point it at something else.

## Summary

Conspectus gets three layers:

| Layer | What it holds | Who writes it | Where |
|---|---|---|---|
| **Graph** | What exists now | Discovery, every cycle | Memory and `graph.bin` (rebuildable cache) |
| **History** | What Conspectus has observed, including entities that are gone, with enough detail to rebuild them | Discovery and launch paths, automatically | `$XDG_STATE_HOME/conspectus/history/` (category 2 observation) |
| **Pins** | Ordered slots the operator keeps at hand: a target plus edits that override what history says | The operator only | `$XDG_DATA_HOME/conspectus/pins.toml` (category 1 intent) |

A **rebuild** turns a recipe back into a live entity. The recipe comes
from history, a pin slot, or both. Kinds that support rebuilding in
this milestone:

- a mux with its launch command, resuming its last session,
- a session, resumed into a mux,
- a linked worktree (repo + branch, through worktrunk),
- a mux whose worktree is gone (the worktree is rebuilt first),
- a repo, by cloning it from its recorded remote.

Two new TUI views share a layout that lists different kinds in aligned
columns:

- **Pinned** lists every pin slot.
- **History** lists every gone entity Conspectus recorded.

Ghost muxes also show inline in the Mux view. The Pins groups at the
top of the Sessions and Mux views go away.

## Decisions from operator review (2026-10-19)

1. **Pins are editable slots.** A slot keeps a reference to its target
   and the edits the operator made. It doesn't depend on history to
   exist (see Pin slots).
2. **History is browsable without pinning**, through a History view,
   with ghost muxes also shown inline in the Mux view.
3. **Rebuild scope for this milestone:** mux plus session resume,
   worktree, mux on a gone worktree, and repo clone.
4. **Pins live in `$XDG_DATA_HOME/conspectus/pins.toml`**, user scope
   only. That extends ADR 0087 category 1 with one file and retires the
   project pin stores and their registry (ADR 0090).

## Pinnability and rebuild by kind

| Kind | Pinnable | What a pin means | History record | Rebuild |
|---|---|---|---|---|
| `MuxSession` | yes | A named terminal workspace: name, cwd, command, last session | yes | `new-session` with the recorded or slot argv, resuming the last session |
| `AgentSession` | yes | A bookmark on a conversation | no; the harness's files are its history, and the mux record lists the sessions each mux hosted | Resume into its last mux (rebuilding the mux if it's a ghost) or into a new mux |
| `Checkout` (linked worktree) | yes | Repo + branch + location | yes | `wt switch <branch>`, or `wt switch --create --base <last head> <branch>` when the branch is gone |
| `Checkout` (primary) | yes | A clone at a path | yes | Repo clone to the recorded path |
| `Repo` | yes | A repository and its remotes | yes | `git clone <remote> <path>` (needs an ADR 0087 amendment) |
| `Branch` | yes | A ref and its last-seen commit | inside the checkout record | Through a worktree rebuild for that branch |
| `ForgePr` | yes | A bookmark on a PR | yes, a display snapshot only | None. A PR never goes away; it can drop out of the open-PR listing |
| `Workspace` | yes | A bookmark | no | None in this milestone (Atelier and agent-deck own materialization) |
| `Fork` | yes | A bookmark | no | None in this milestone |
| `RuntimeProcess` | **no** | Ephemeral | no | — |
| `Pin` | no | The slot itself | — | — |

A bookmark-only pin still gets a Pinned row, Reveal, and the target's
detail pane. A bookmark whose target disappears shows as missing,
using the slot's snapshot (see below).

## History

### Records

History is a directory of JSON files, one per kind, sharing one lock:

```
$XDG_STATE_HOME/conspectus/history/
  history.lock
  muxes.json
  checkouts.json
  repos.json
  prs.json
```

This is the "last-seen index" that `docs/design.md` State And
Persistence already allows as global state. It isn't kept in
`graph.bin`, because `graph.bin` is thrown away on every format bump
(ADR 0082) and history can't be rebuilt once the entity is gone.

Every record has a common envelope plus a recipe specific to its kind:

```json
{
  "id": "h_7f3a9c",
  "key": "tmux:conspectus-main",
  "first_seen": 1760000000,
  "last_seen": 1760600000,
  "origin": "observed",
  "display": { "label": "conspectus-main", "context": "claude · ~/src/conspectus" },
  "recipe": { "...": "kind-specific, below" }
}
```

- `id` is a stable random id. Pins point at records by this id, so a
  record can change its key (an external tmux rename, a worktree that
  moved) without anyone rewriting `pins.toml`. The daemon can't write
  `pins.toml` (ADR 0087 prohibition 5).
- `key` is the graph `NodeId` string the record last matched.
- `display` is what the History and Pinned views show once the entity
  is gone.
- `origin` is `observed`, `launched`, or `migrated`.

Recipes by kind:

| Kind | Recipe fields |
|---|---|
| Mux | backend, socket, name, cwd, incarnation (`session_id`, `created`), launch argv and its source, hosted sessions (harness, id, first and last seen; the 10 most recent) |
| Checkout | repo record id, root, linked or primary, branch, head commit, upstream |
| Repo | common dir, remotes (fetch URLs), primary checkout root, default branch |
| PR | provider, host, owner, repo, number, title, state, URL, head ref, last listed |

Launch argv is recorded verbatim, including the `pane_start_command`
of muxes started outside Conspectus, even though a harness argv can
carry a prompt. Conspectus already reads comparably sensitive state, so
the history ADR amends ADR 0086's Tier 3 rule rather than adding a
scrubber (decision 6). Nothing else in a record is payload.

### Presence

The graph gains a sparse `node_presence` map next to `node_provenance`:

- **Live**: discovered this cycle. This is the default, so most nodes
  have no entry.
- **Gone** `{ last_seen }`: a targeted check of the recorded entity
  failed.
- **Unknown**: the check couldn't run.

**Absence from a scan never means gone.** Discovery's scope depends on
scan roots, on which sockets are enumerated, and on whether the forge
was queried this cycle. A TUI started with a different scan root must
not mark a repo's worktrees gone. Gone takes a targeted negative check
of the record itself:

| Kind | Live | Gone | Unknown |
|---|---|---|---|
| Mux | Listed by a successful probe of its backend and socket | That probe succeeded and didn't list it. "No server running" counts as success: after a reboot every mux really is a ghost | tmux missing, probe failed, or the socket isn't enumerated (the ledger's sockets join the probe set, absorbing part of CSP-382) |
| Checkout | Root exists and is a checkout of the recorded repo | Root is missing and its parent directory exists | Parent is missing too (an unmounted drive), or permission errors |
| Repo | Common dir exists | Common dir is missing and its parent exists | As for checkouts |
| PR | Returned by the latest listing | Never gone; state comes from the last listing that returned it | — |

The checks are cheap (`stat` plus the probes discovery already runs),
so the history provider runs them every cycle for every record.

### Identity and matching

For muxes, when a probe lists one, the writer matches it to a record in
this order:

1. **Same incarnation** (backend, socket, tmux `#{session_id}` and
   `#{session_created}`): the same session, possibly renamed outside
   Conspectus, so the record's name and key follow it. This heals what
   `pin rebind` fixes by hand today.
2. **Same key, record is a ghost**: the name came back (a rebuild, or
   recreated by hand), so the record comes back to life with a new
   incarnation. Hosted sessions without activity after the new
   `created` don't carry over (ADR 0102).
3. **Otherwise** a new record.

Zellij has no session id and matches on rule 2 only.

Checkouts match on the checkout id (repo + root). A rebuilt worktree at
a different path is a new record that links back with `rebuilt_from`.
Repos match on common dir, and a re-clone to the same path re-lives the
record. PRs match on PR id.

### Writers and concurrency

- **The daemon** writes at the end of each cycle, for the provider
  classes it just refreshed.
- **In-process discovery** (TUI or CLI with no daemon) writes at the
  point where it would write `graph.bin`.
- **Rebuild and launch paths** write the record (with argv) when they
  create the entity, without waiting for the next probe.
- A client of a running daemon doesn't discover, so it doesn't write.

Each writer:

1. takes an exclusive advisory lock on `history.lock`
   (`std::fs::File::lock`, available on the 1.99 toolchain, so no new
   dependency),
2. re-reads the files,
3. merges in only the observations it made itself, and
4. writes through `write_atomic`.

The merge is commutative:

- `first_seen` takes the minimum and `last_seen` the maximum,
- hosted-session lists are unioned and then truncated,
- missing records are created.

So any number of processes can write in any order, and a reap can't be
undone by a writer holding an old copy. Only a fresh sighting of the
entity brings back a reaped record, which is correct.

Only fresh probes count. A warm-start `graph.bin`, a daemon snapshot,
or a discovery cache from an earlier cycle (ADR 0099) never moves
`last_seen`.

`last_seen` for a live entity is rewritten at most every 10 minutes
unless something else in the record changed. Reap thresholds are in
days, so that precision is enough. A mux created and killed between two
probes is never recorded. With the daemon polling muxes every few
seconds that's rare, and catching it would take a tmux hook in the
operator's tmux config, which is outside the envelope.

`--no-cache`, the fixture and snapshot modes (ADRs 0067, 0068, 0069),
the dev scenario builder, and tests with no state home don't write
history. Fixtures can carry history, so gone entities are
snapshot-testable.

### Mutation envelope

History is category 2 (rebuildable observation under
`$XDG_STATE_HOME/conspectus/`), which `docs/capabilities.toml` already
allows in the `read` context. The daemon already writes `graph.bin`, so
it can evidently write category 2 files. ADR 0090 read prohibition 5
more strictly. The history ADR should state the rule outright: the
daemon may write category 2 observations and never categories 1, 3, 4,
or 5.

Deleting history loses gone entities and their recorded commands. It
loses no operator intent: pin slots keep their own copy of what they
need.

### Ignore rules and reaping

- `[history.ignore]` takes globs per kind (`mux = ["scratch*"]`).
  Matching entities are never recorded.
- `conspectus history reap [--kind <k>] [--older-than <dur>]
  [--dry-run]` removes gone records past a per-kind threshold
  (`[history.reap_after]`). `conspectus history forget <ref>` removes
  one record.
- Records a pin slot targets are never reaped. Neither are records in
  the Unknown state.
- Reaping is operator-initiated only. It removes history records and
  never touches tmux, git, or the filesystem.
- Worktree close-down (`worktree close`), which drops the worktree's
  pins today, instead forgets the gone records rooted in the worktree.

## Pin slots

### Shape

A slot is a named, editable place the operator keeps at hand. It has
three parts:

- **target**: what it points at. That's a history record id for kinds
  with history, a `NodeId` for the rest, or nothing for a *planned* slot
  (decision 3).
- **overrides**: recipe fields the operator set. These are intent and
  win over everything else.
- **snapshot**: the recipe and display copied from history when the
  slot was created or last edited. It lets the slot render and rebuild
  if history is lost.

```toml
[pins]
schema_version = 2

[[pins.slots]]
id = "conspectus-main"
kind = "mux"
target = "history:h_7f3a9c"
pinned_at = 2026-10-19T12:00:00Z

[pins.slots.overrides]
argv = ["claude", "--dangerously-skip-permissions"]

[pins.slots.snapshot]
name = "conspectus-main"
cwd = "/home/me/src/conspectus"
harness = "claude-code"
label = "conspectus-main"

[[pins.slots]]
id = "ledger-worktree"
kind = "checkout"
target = "history:h_19be02"

[pins.slots.overrides]
branch = "feature/ledger"

[pins.slots.snapshot]
repo = "/home/me/src/conspectus/.git"
root = "/home/me/wt/conspectus.feature-ledger"
head = "4630e1e"

[[pins.slots]]
id = "ledger-pr"
kind = "pr"
target = "pr:github:github.com:malloc47:conspectus:612"

[pins.slots.snapshot]
label = "#612 Ledger store and merge"
```

The effective recipe is resolved field by field:
**overrides > live graph > history > snapshot > harness or kind
defaults.**

The snapshot is written only by operator gestures: pin, edit, retarget,
and "refresh snapshot". The daemon never writes `pins.toml`, so a
snapshot can be old. That's acceptable because history and the live
graph come before it.

`schema_version = 2` stops an older binary from loading the new table
(ADR 0057's rule for unknown versions) instead of misreading it.

### Editing

| Gesture | Effect |
|---|---|
| Pin (from any row, live or gone) | Creates a slot targeting that entity, with a snapshot and no overrides. `id` defaults to the target's display name, made unique |
| Edit | Sets or clears overrides through a form for each kind (reusing the launch-spec form, ADR 0097) |
| Rename | Changes the slot `id` and label. On a live mux slot, also renames the tmux session in lockstep (ADR 0029) and sets the `name` override |
| Retarget | Points the slot at another entity of the same kind, live or from history. This replaces `pin rebind` and `pin adopt` |
| Refresh snapshot | Copies the current recipe from the graph or history into the snapshot |
| Unpin | Removes the slot. The target's history record becomes reapable again |

Which fields each kind can override (decision 2):

| Kind | Overridable |
|---|---|
| Mux | name, socket, cwd, harness, argv, worktree branch (the cwd then comes from that branch's worktree) |
| Session | resume target: the mux to resume into, by name or slot |
| Checkout | branch, base for a fresh branch, location (advisory; decision 4) |
| Repo | remote, clone path |
| PR, branch, workspace, fork | label only |

Session slots follow lineage. A session that compacts or resumes
(ADR 0018) keeps its slot on the lineage head.

### Graph projection

`PinNode` (ADR 0084) stays as the slot's graph identity (`pin:<id>`),
so Pinned rows have a detail target and slots show up in search and
graph exports. It changes in three ways:

- it carries `kind`, the target, the overrides, and the effective
  recipe,
- `pin_targets_mux` generalizes to `pin_targets`, which points at any
  node, including a gone one,
- `pin_realized_by_session` goes away, because a mux slot's session is
  the mux's session.

The pin binding pass (ADR 0057, ADR 0102) goes away. A slot's target
is an explicit reference; there's nothing to infer. Session-to-mux
attribution stays with the general resolver. ADR 0102's stickiness (the
previous binding as the weakest evidence) moves to every mux, through a
`Cached` `linked_to_mux` fallback built from the mux record's latest
hosted session. That is the "Option A" ADR 0058 deferred. The four
`Pin*` diagnostics give way to presence and to the slot statuses below.

### Native views

Native views stop floating pinned entities. A pinned entity gets a
one-cell pin marker and stays where it is, and a `pinned only` filter
is added to Controls. Floating is the Pinned view's job.

## Rebuild

A rebuild is a **plan**: an ordered list of steps that is shown before
it runs, and that `--dry-run` prints. Each kind contributes an
executor, and an executor can declare prerequisites:

```
rebuild mux "conspectus-feature" (ghost)
  1. rebuild worktree feature/ledger      wt -C ~/src/conspectus switch feature/ledger
  2. new tmux session                      tmux new-session -d -s conspectus-feature -c <worktree> …
  3. resume session e44d01cb               claude --dangerously-skip-permissions --resume e44d01cb
  4. attach
```

| Executor | Steps | Envelope |
|---|---|---|
| Mux | `new-session` with the effective argv, resume spliced after the harness binary (ADR 0098), a failed resume falls back to a fresh launch (ADR 0103), then attach | Category 3, unchanged |
| Mux, agent exited (live mux, no harness) | Attach if pane-process evidence shows the harness running, otherwise `send-keys` the Conspectus-built argv (ADR 0102, ADR 0028) | Category 3, unchanged |
| Session | Its last mux's executor with this session as the resume target, or a new mux when that mux is live and busy (CSP-170) | Category 3 |
| Worktree | `wt switch <branch>`; when the branch is gone, `wt switch --create --base <last head> <branch>`. Prerequisite: the repo, when it's gone | Category 4 via worktrunk (ADR 0092), manifest unchanged |
| Primary checkout or repo | `git clone <remote> <path>` into a path that doesn't exist | **New**: an ADR superseding ADR 0087, plus a manifest entry (decision 5) |
| Mux on a gone worktree | Worktree executor, then the mux executor | Composition; replaces worktree-backed pins (ADR 0094) |

Argv precedence for a mux:

1. the slot's `argv` override,
2. the history record's recorded argv,
3. the project launch profile (`[launch.<harness>]` in `.conspectus.toml`),
4. the user launch profile (`[launch.<harness>]` in the user config),
5. a discovered `atelier exec <harness>` wrapper, when the cwd is inside
   an Atelier workspace and `atelier` is on `PATH` (decision 7),
6. `HarnessAdapter::launch_argv`.

A mux with no harness (a shell, `emacsclient -c`, `conspectus serve`)
rebuilds with its recorded argv, or with a shell when none was
recorded.

Rebuilding from history or from the History view needs no pin. A mux's
History entry expands to its hosted sessions, each with "rebuild with
this session".

Orchestrator-owned muxes (agent-deck) are recorded and shown, but their
rebuild is disabled ("owned by agent-deck"), because the orchestrator
owns their lifecycle (CSP-451).

Zellij: an `EXITED` session is a ghost, and its rebuild runs
`zellij attach` to resurrect it, falling back to creation if zellij has
dropped it. This needs CSP-618 first.

## Pinned and History views

### Row layout for mixed kinds

Both views render rows of different kinds in one set of aligned columns,
following ADR 0131's rule: fixed columns pad to the widest value in the
whole tree, and content fills the rest of the row and is cut with an
ellipsis.

```
<indent><disclosure?> <kind> <label>  <status>  <age>  <content>
```

| Column | Width | Source |
|---|---|---|
| kind | 1 cell | ADR 0073 kind glyph, shown for **every** kind, including sessions (ADR 0073 suppresses it in single-kind row contexts; here it's the only kind signal, so it doesn't double one) |
| label | padded, capped at 28 | Slot id in Pinned; the record's display label in History |
| status | padded, at most 9 | Kind vocabulary below |
| age | right-aligned, padded | Live: last activity. Gone: last seen |
| content | flexible, truncated | The kind's context, below |

| Kind | Status values | Content |
|---|---|---|
| Mux | `live`, `exited` (no harness), `gone`, `unknown`, `planned` | `harness · ~/cwd` or the command |
| Session | `in mux`, `resumable`, `missing` | `harness · title` |
| Checkout | `live`, `gone`, `planned` | `repo · branch` |
| Repo | `live`, `gone` | first remote |
| Branch | `live`, `gone` | `repo · upstream` |
| PR | `open`, `draft`, `merged`, `closed` | `owner/repo · title` |
| Workspace, fork | `live`, `gone` | provider · root |

Gone, missing, and planned rows render dimmed. That replaces the
"ghost glyph" of the first revision: the status column carries
presence, so no new glyph is needed.

```
Pinned · by project
▼ ~/src/conspectus
  ▶ ▣ conspectus-main    live       2m  claude · ~/src/conspectus
    ▣ conspectus-serve   live      40s  conspectus serve
    ● e44d01cb           resumable 17m  claude · Fix the ledger merge
    ◇ ledger-worktree    gone       2d  conspectus · feature/ledger
    ⇄ ledger-pr          open       5h  malloc47/conspectus · Ledger store and merge
▼ ~/src/config
    ▣ config-agent3      gone       3d  claude · ~/src/config
```

### Pinned view

- Rows are slots; selecting one selects its `PinNode`, and the detail
  pane shows the slot, its effective recipe, which layer supplied each
  field, and the target.
- Grouping: `project` (default; a slot's repo is derived from its
  target or cwd, and slots without one go in Other), `kind`, or `none`.
  Sort: slot order by default, or recency. New slots append to the
  order, `pin move` changes it, and reordering from the TUI is a
  follow-up (decision 8).
- Mux slots expand to their sessions, and checkout slots to their muxes
  and sessions.
- `Enter` runs the kind's primary action: attach to a live mux, rebuild
  a gone one, resume a session, open a mux in a live worktree or
  rebuild a gone one, open a PR. The row menu holds edit, rename,
  retarget, refresh snapshot, unpin, reveal in the native view, and
  show the rebuild plan.

### History view

- Rows are gone records, in the same layout, grouped by project by
  default and sorted by last seen.
- `Enter` rebuilds. The menu holds pin, forget, and show the rebuild
  plan. A "Clean up" modal lists reap candidates with their ages and
  lets the operator deselect any before confirming (menu-first, per the
  TUI discoverability preference).

### Mux view

Ghost muxes sort below live muxes in each group, dimmed, with the
`gone` status and their last-seen age. A Controls toggle hides them.
The header reads `11 mux · 4 gone`.

### Removed

- The Pins groups in the Sessions and Mux views.
- The unbound-pin placeholder rows in both views. A historical session
  already has its own row, and it gains a "was in `conspectus-main`"
  chip.
- The Pins modal (`p`) and the `N` / `B` / `b` / `A` pin shortcuts.
  `N` becomes "mux launch with worktree" (ADR 0096). Pin and unpin
  become row actions in every view, menu-first, with one accelerator
  key. This makes the Pins half of CSP-619 moot.

## CLI

| Command | Status |
|---|---|
| `history list [--kind <k>] [--gone]`, `history show <ref>` | New |
| `history reap`, `history forget <ref>` | New |
| `rebuild <ref> [--dry-run] [--no-attach]` | New. `<ref>` is a slot id, a history id, or a node reference |
| `pin add <ref> [--name <id>]` | New. Replaces `pin create` and `pin adopt` |
| `pin edit <id> --set <field>=<value> / --unset <field>` | New |
| `pin rename <id> <new-id>` | Kept, with lockstep for mux slots |
| `pin retarget <id> <ref>` | New. Replaces `pin rebind` |
| `pin refresh <id>`, `pin rm <id>`, `pin list`, `pin show <id>` | Kept or new |
| `pin launch`, `pin attach` | Retired in favor of `rebuild <slot>` |
| `pin bind` | Retired in favor of `declared create` (ADR 0014), which is what it wrote |
| `pin migrate [--dry-run]` | New, one-time |
| `mux new`, `mux launch` | Unchanged, plus a history write with argv. `--pin` pins the result in the same gesture |

## Migration

Today's pins map onto mux slots one-to-one, so migration loses nothing.
`conspectus pin migrate` (operator-run, since it writes category 1):

1. reads every `[[pins.entries]]` in the project stores (through the
   ADR 0090 registry) and in the user store,
2. writes one mux slot each, keeping the `id`, with overrides for
   `mux.name`, `socket_name`, `cwd`, `harness`, `launch.argv`, and
   `worktree.branch` wherever they differ from defaults,
3. seeds a history record per slot, with `last_session` taken from the
   ADR 0058 sidecar, and targets the slot at it,
4. removes the converted `[[pins.entries]]` tables, keeping comments
   and formatting (`toml_edit`), and
5. prints what it did. `--dry-run` prints the plan only.

Until the operator migrates, discovery treats legacy entries as
in-memory slots, so nothing disappears on upgrade, and a `LegacyPins`
diagnostic suggests the command. One release later, the legacy loader,
the pin-binding sidecar directory, and `pin-stores.json` are removed.

Migrating every pin is fine now, because pinned items no longer crowd
the native views. The operator can unpin what history now covers.

## Implications

### ADRs

| ADR | Content | Supersedes or amends |
|---|---|---|
| New: history and presence | Records, presence, matching, writers, merge, reaping, ignore rules, verbatim argv | Supersedes 0058 and 0102; amends 0057 (binding) and 0086 (verbatim argv in Tier 3) |
| New: pin slots | Slot shape, precedence, editing, `pins.toml` location (category 1 extension), `PinNode` and `pin_targets` | Supersedes 0057 (schema, CLI, TUI) and 0090; amends 0084 |
| New: rebuild plans | Executors, prerequisites, argv precedence, worktree location rule | Supersedes 0094; amends 0096 and 0095 ("adopt as a pin" goes away) and 0093 (teardown leaves a ghost) |
| New: mutation envelope | Restates ADR 0087 and adds the clone carve-out, the daemon's category 2 writes, `pins.toml`, and the history store | Supersedes 0087 |
| New: Pinned and History views | Row layout for mixed kinds, statuses, grouping, actions, native-view pin marker | Amends 0073 (kind glyph shown in mixed views) |

`docs/design.md` gets rewritten Session Pins and Mux Lifecycle sections
and a History section under State And Persistence, and its Decisions
are updated. `docs/operations.md`, `docs/pins-walkthrough.md`, the
README, and the showcase scenario (ADR 0070) are rewritten.

### Code

- **Replaced**: `src/pins.rs`, `src/discovery/pins.rs`,
  `src/resolve/pins.rs`, `src/pin_bindings.rs`,
  `src/pin_store_registry.rs`, and most of `src/cli/pin.rs` (1,455
  lines). Its launch logic moves into the mux rebuild executor.
- **New**: a history module (records, lock, merge, presence checks), a
  pin slot module, and a rebuild module (plans and executors).
- **Model**: `node_presence`, `PinNode` reworked, `pin_targets`, and
  the `rebuilt_from` and `mux_last_hosted` relations; the `Pin*`
  diagnostics removed; snapshot format bump.
- **Discovery**: tmux list format gains `#{session_id}`; mux probes
  report status per socket; a history provider emits gone nodes and
  runs presence checks.
- **Server**: history write at the end of each cycle.
- **TUI**: a row renderer for mixed kinds, the Pinned and History views,
  ghost rows in the Mux view, slot edit and retarget forms; the Pins
  groups, placeholder rows, and Pins modal removed.
- **Audit**: every consumer that assumes a node exists now (attach,
  rename, teardown, mux chips, "float muxed to top", header counts, the
  linkers, worktree actions, Reveal) must handle gone nodes. This is the
  riskiest part of the change.
- **Tests**: sparse and ambiguous cases for name reuse, external
  rename, probe failure, an unenumerated socket, an unmounted path,
  concurrent writers, a reap racing a writer, a dangling target, a
  planned slot, field precedence, and compound rebuild plans.

### Existing backlog

| Task | Effect |
|---|---|
| CSP-617 (`pin adopt` misses live tmux) | Obsolete; close when `adopt` is retired |
| CSP-618 (zellij pins launch through tmux) | Blocks the zellij ghost rebuild |
| CSP-382 (non-default socket discovery) | Partly absorbed: the ledger supplies the socket set |
| CSP-385 (glob pin patterns) | Obsolete |
| CSP-455, CSP-455.01, CSP-434 (pin create form cwd picker) | Retarget to the mux launch and slot edit forms |
| CSP-619 (Controls and Pins overlay contract) | Pins half moot |
| CSP-170 (resume an un-muxed session into a mux) | Becomes the session rebuild executor |
| m-20 (0.1.0) | Ships first; this milestone is 0.2.0 (decision 9) |

### Risks

- **Gone nodes leaking into live semantics.** Handled by the audit,
  presence-aware `SnapshotIndex` helpers, and fixtures with gone nodes.
- **False gone** from a stale snapshot, a failed probe, scan-root
  differences, or an unmounted drive. Handled by the targeted-check
  rule and the Unknown state; Unknown is never reaped.
- **Worktree rebuild landing at a different path**, because worktrunk's
  path template decides where (decision 4, CSP-688).
- **Clone widens the envelope.** Kept narrow: act context only, never
  into an existing path, no other git commands.
- **History loss** drops unpinned gone entities. Pinned ones survive
  through their snapshots.

## Decisions (2026-10-21)

1. **Slot field precedence:** overrides > live graph > history >
   snapshot > defaults. Snapshots are written only by operator
   gestures, never by the daemon.
2. **Overridable fields** as in the Editing table. Mux slots keep
   editable `harness` and `cwd`, as pins have today.
3. **Planned slots are kept.** A slot with no target is today's "pin
   without launching", built on its first rebuild.
4. **Worktree location:** accept worktrunk's path for now. The new
   record links to the old one with `rebuilt_from`, and slots move to
   it. CSP-688 investigates why recorded roots and worktrunk's template
   diverge, and whether exact locations should be honored.
5. **Repo clone** uses `git clone`, under a new ADR that **supersedes**
   ADR 0087. That ADR also states the daemon's category 2 rule and adds
   the `pins.toml` store.
6. **Argv is recorded verbatim**, including commands of muxes started
   outside Conspectus. No prompt scrubbing; the history ADR amends
   ADR 0086 Tier 3.
7. **Launch profiles are kept**, user scope with project override,
   plus discovery of `atelier exec`: inside an Atelier workspace with
   `atelier` on `PATH`, the default argv wraps the harness. (This
   reading of "discovery of `atelier exec`" is recorded in CSP-683 to
   confirm when it's picked up.)
8. **Slot order** is part of the store model. The Pinned view sorts by
   it, `pin move` changes it, and TUI reordering is CSP-687.
9. **Version:** the milestone ships as 0.2.0 after 0.1.0 (CSP-689),
   breaking the `pin` CLI cleanly. Legacy pins load until `pin migrate`
   runs, and the legacy loader is removed one release later (CSP-690).
10. **Session names** fall back to `alias > title > mux name >
    id-suffix`. Slot ids show only in the Pinned view, and slot names
    are expected to match mux names.

Also adopted as recommended:

- Every mux is recorded, with ignore globs.
- Mux matching order: same incarnation, then a ghost with the same
  key, then a new record.
- Merge under a lock rather than a single elected writer.
- `last_seen` precision of 10 minutes, labeled "last seen".
- Reaping is manual, with per-kind thresholds (muxes 14 days,
  checkouts 30 days, PRs 30 days after leaving the listing, repos only
  by `forget`).
- The History view shows gone records only.
- Native views show a pin marker and a `pinned only` filter, without
  floating pinned items.
- Zellij `EXITED` sessions are ghosts.
- Orchestrator-owned muxes are recorded with rebuild disabled.
- Bulk rebuild after a reboot is a follow-up (CSP-686).
- Pinned PRs that leave the listing are refreshed lazily (CSP-691).

## Milestone and tickets

Milestone **m-23, History, Rebuild, And Pin Slots**. Every task carries
the `history-pins` label, and dependencies are recorded in Backlog.

| Area | Tasks |
|---|---|
| Decisions | CSP-656 history and presence ADR · CSP-657 pin slots ADR · CSP-658 rebuild ADR · CSP-659 envelope ADR superseding 0087 · CSP-660 Pinned and History views ADR · CSP-661 `docs/design.md` |
| History | CSP-662 store · CSP-663 mux probe status · CSP-664 mux recording · CSP-665 checkout, repo, and branch recording · CSP-666 PR snapshots · CSP-667 graph projection and live-assumption audit · CSP-669 history commands and reaping |
| Pins | CSP-668 slot store and model · CSP-670 slot CLI · CSP-671 migration |
| Rebuild | CSP-672 engine and mux executor · CSP-673 sessions · CSP-674 worktrees · CSP-675 repo clone · CSP-676 prerequisite chaining · CSP-677 zellij ghosts (after CSP-618) |
| TUI | CSP-678 row renderer for mixed kinds · CSP-679 Pinned view · CSP-680 History view and Clean up · CSP-681 Mux view ghosts · CSP-682 pin actions everywhere, old pin surfaces removed |
| Follow-through | CSP-683 launch profiles with `atelier exec` · CSP-684 mux-name display fallback · CSP-685 docs and fixtures · CSP-689 0.2.0 release (after CSP-541) |
| Later | CSP-686 bulk rebuild · CSP-687 TUI slot reordering · CSP-688 worktree location investigation · CSP-690 retire the legacy loader · CSP-691 pinned PR refresh |

Order of work:

1. The decisions (CSP-656..CSP-661).
2. The history spine: CSP-662 → CSP-663 → CSP-664, with CSP-665 and
   CSP-666 alongside, then CSP-667.
3. Slots (CSP-668) and the rebuild engine (CSP-672), then executors,
   CLIs, and migration in parallel.
4. The TUI: CSP-678 first, then the views.
5. Docs, then the 0.2.0 release.

Existing tasks affected:

- CSP-617 closes as obsolete when `pin adopt` is retired (CSP-670).
- CSP-170 is covered by CSP-673.
- CSP-618 blocks CSP-677.
- CSP-382 is partly absorbed by CSP-663.
- CSP-385 is obsolete.
- CSP-455, CSP-455.01, and CSP-434 retarget to the mux launch and slot
  edit forms.
- The Pins half of CSP-619 goes away with CSP-682.
