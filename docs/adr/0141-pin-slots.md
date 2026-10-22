# ADR 0141: Pin Slots

## Status

Accepted. Supersedes ADR 0057's storage schema, store selection, CLI,
and TUI sections, and ADR 0090 (the pin-store registry). Amends ADR
0084 (`PinNode` and its relations). With ADR 0140, nothing of ADR
0057's binding model remains. Part of the history, rebuild, and pin
slots plan with ADRs 0140, 0142, 0143, and 0144.

## Context

ADR 0057 made a pin a declared `(harness, cwd, display_name, mux)`
tuple in `[[pins.entries]]`, stored in the nearest project's
`.conspectus.toml` or the user config, bound to a live session each
cycle, and launched with `pin launch`. Because pins were the only way
to make a mux survive its death, they turned into a list of every mux
the operator runs, and they could only describe muxes.

History (ADR 0140) now records every rebuildable entity, so a pin no
longer has to carry a mux's lifecycle. The operator still wants pins
as editable slots, as they are today (rename one, change its launch
command, point it at something else), and wants to pin anything that
could be rebuilt, not only muxes. A pin should mean "keep this at
hand", not "make this survive".

Project-local pin stores also cost a registry (ADR 0090) to find them
outside the scan roots, and a pin's store depended on where its cwd
was.

## Decision

### A slot

A **pin slot** is a named, ordered, editable place the operator keeps
at hand. It has three parts:

- **target**: what it points at. A history record (`history:<id>`)
  for kinds whose identity can change (muxes, checkouts, repos), a
  graph node id for the rest (`agent_session:…`, `forge_pr:…`,
  `branch:…`, `workspace:…`, `fork:…`), or nothing, for a **planned**
  slot that will be built on its first rebuild (today's "pin without
  launching").
- **overrides**: recipe fields the operator set. They are intent and
  win over everything else.
- **snapshot**: the recipe and display copied when the slot was
  created or last edited, so the slot renders and rebuilds even if
  history is lost.

The store is one user-scope file, `$XDG_DATA_HOME/conspectus/pins.toml`
(falling back to `~/.local/share/conspectus/pins.toml`), category 1
under ADR 0143:

```toml
[pins]
schema_version = 2

[[pins.slots]]
id = "conspectus-main"
kind = "mux"
target = "history:h_7f3a9c0e21d4"
pinned_at = 2026-10-19T12:00:00Z

[pins.slots.overrides]
argv = ["claude", "--dangerously-skip-permissions"]

[pins.slots.snapshot]
name = "conspectus-main"
cwd = "/home/me/src/conspectus"
harness = "claude-code"
label = "conspectus-main"

[[pins.slots]]
id = "ledger-pr"
kind = "pr"
target = "forge_pr:github:github.com/malloc47/conspectus#612"

[pins.slots.snapshot]
label = "#612 Ledger store and merge"
```

- `id` is unique, non-empty, and is the slot's name. The Pinned view
  shows it; nothing else does (ADR 0144).
- `kind` is one of `mux`, `session`, `checkout`, `repo`, `branch`,
  `pr`, `workspace`, `fork`. Runtime processes are ephemeral and can't
  be pinned, and neither can a pin.
- **Order is the order of the `[[pins.slots]]` tables.** New slots are
  appended, `pin move` moves one, and the Pinned view sorts by it.
- `schema_version = 2` makes an older binary skip the table (ADR
  0057's rule for unknown versions) instead of misreading it.
- Writes go through `toml_edit`, change only the slot they touch, and
  keep the operator's comments, unknown keys, and formatting.

### Effective recipe

Each recipe field resolves on its own, from the first source that has
it:

**overrides > live graph > history > snapshot > kind or harness
defaults**

The snapshot is written only by operator gestures: pin, edit,
retarget, and "refresh snapshot". The daemon never writes `pins.toml`
(ADR 0143), so a snapshot can be old, which is acceptable because the
live graph and history come before it.

| Kind | Overridable fields |
|---|---|
| Mux | `name`, `socket`, `cwd`, `harness`, `argv`, `worktree_branch` (the cwd then comes from that branch's worktree) |
| Session | `resume_into`: the mux to resume into, by name or slot id |
| Checkout | `branch`, `base` (for a fresh branch), `location` (advisory, ADR 0142) |
| Repo | `remote`, `path` |
| PR, branch, workspace, fork | none; renaming the slot is the only edit |

A session slot follows lineage (ADR 0018): a session that compacts or
resumes keeps its slot on the lineage head. The stored target changes
only when the operator refreshes or retargets the slot.

### Status

A slot's status comes from its target: the target's presence (live,
gone, unknown; ADR 0140), **missing** when the target is in neither
the graph nor history (it renders from the snapshot), or **planned**
when it has no target. Kinds refine it in the views (ADR 0144).

### Gestures

| Gesture | Effect |
|---|---|
| Pin (any pinnable row, live or gone) | A slot targeting that entity, with a snapshot and no overrides. `id` defaults to the target's display name, made unique |
| Edit | Sets or clears overrides, through a form for each kind |
| Rename | Changes the slot `id`. On a live mux slot, also renames the mux in lockstep (ADR 0029) and sets the `name` override |
| Retarget | Points the slot at another entity of the same kind, live or from history. Replaces `pin rebind` and `pin adopt` |
| Refresh snapshot | Copies the current effective recipe into the snapshot |
| Move | Changes the slot's place in the order |
| Unpin | Removes the slot. Its history record becomes reapable again |

A history record a slot targets is never reaped (ADR 0140).

### Graph projection

`PinNode` (ADR 0084) stays as the slot's graph identity, `pin:<id>`,
so Pinned rows have a detail target and slots appear in search and
exports. It now carries the slot's kind, target, overrides, status,
and effective recipe with the source of each field. Its relations:

- `pin_targets`, from the slot to its target node, gone or live.
  Replaces `pin_targets_mux`. A missing target is an unresolved
  endpoint.
- `pin_realized_by_session` goes away: a mux slot's session is the
  mux's session.

The pin binding pass and its four diagnostics go away (ADR 0140). A
pin's name no longer aliases a session: session display names fall
back `alias > title > mux name > id-suffix`, where the mux is the live
one or the ghost that last hosted the session, so an untitled session
in a named mux reads as the mux's name.

Native views stop floating pinned entities. A pinned entity gets a
one-cell pin marker where it is, and Controls gains a `pinned only`
filter (ADR 0144).

### Legacy pins and migration

Today's `[[pins.entries]]` map one-to-one onto mux slots.
`conspectus pin migrate [--dry-run]`, which the operator runs because
it writes category 1:

1. reads every legacy entry in the project stores (through the ADR
   0090 registry and the scan roots) and in the user store,
2. writes one mux slot each, keeping its `id`, with overrides for the
   mux name, socket, cwd, harness, launch argv, and worktree branch
   where they differ from the defaults,
3. seeds a history record per slot (`origin = "migrated"`), with the
   last session taken from the ADR 0058 sidecar, and targets the slot
   at it,
4. writes an alias where a pin's `display_name` differed from both its
   mux name and its session's title, so the name it showed survives,
5. removes the converted entries, keeping comments and formatting, and
6. prints what it did. `--dry-run` prints the plan only, and a second
   run finds nothing to do.

Until the operator migrates, discovery loads legacy entries as
in-memory slots, so nothing disappears on upgrade, and a `LegacyPins`
diagnostic names the command. One release later the legacy loader,
the pin-binding sidecar directory, and `pin-stores.json` are removed
(backlog `CSP-690`).

### CLI

| Command | Effect |
|---|---|
| `pin add <ref> [--id <id>]` | Pin a slot id, history id, or node reference. Replaces `pin create` and `pin adopt` |
| `pin edit <id> --set <field>=<value> / --unset <field>` | Edit overrides |
| `pin rename <id> <new-id>` | Rename, with mux lockstep |
| `pin retarget <id> <ref>` | Replaces `pin rebind` |
| `pin refresh <id>`, `pin move <id> --before/--after <id>`, `pin rm <id>`, `pin list`, `pin show <id>` | As named |
| `pin migrate [--dry-run]` | One-time migration |
| `mux new` / `mux launch … --pin [--no-launch]` | Pin the result in the same gesture, or write a planned slot |

`pin create`, `pin launch`, `pin attach`, `pin bind`, `pin rebind`, and
`pin adopt` are retired with the 0.2.0 version bump. Each fails with a
message naming its replacement (`rebuild`, `declared create`, `pin
retarget`, `pin add`) rather than keeping a shim.

## Consequences

- Pinning means "keep at hand" again. Unpinning a mux loses nothing,
  because history still rebuilds it.
- Any kind but a runtime process can be pinned, and a pin can point at
  something that's gone.
- One store, no registry, no dependence on scan roots or on where a
  pin's cwd is.
- Pins are no longer project files, so they can't be committed with a
  project. Nothing used that, and the user-scope store fits slots that
  can point across projects.
- The binding pass, its diagnostics, its sidecar, and its registry
  leave the code; `rebuild` replaces `pin launch` (ADR 0142).
- The `pin` CLI breaks with the 0.2.0 version bump, deliberately.

## Alternatives Considered

- **Pins as pure references to history records.** The plan's first
  revision. Rejected on review: the operator edits pins, retargets
  them, and plans muxes that don't exist yet, which a pure reference
  can't hold.
- **Keep project-local pin stores.** They need the registry to stay
  visible outside the scan roots, and a slot can point at an entity in
  another project, or none.
- **Store slots in `config.toml`.** It is hand-edited configuration,
  often kept in dotfiles. Slots are written mostly by gestures and
  carry snapshots that change; mixing them would churn the operator's
  config.
- **A daemon-refreshed snapshot.** Would keep snapshots current but
  makes the daemon write category 1, which ADR 0143 forbids. Precedence
  puts the live graph and history ahead of the snapshot instead.
- **An explicit `order` number per slot.** Two slots could share a
  number, and every move would rewrite several slots. Table order is
  the order the operator reads in the file.

## Open Questions Answered

- *Does a pin still alias its session?* No. Session names fall back to
  the mux name, and migration writes aliases where a pin's name would
  otherwise be lost.
- *What happens to a slot whose target is reaped or never seen again?*
  It reads `missing` and renders from its snapshot; a rebuild uses the
  snapshot's recipe.
- *Can the daemon fix a stale snapshot?* No. Precedence makes that
  unnecessary, and "refresh snapshot" is one gesture.

## Related ADRs

- ADR 0014 — declared links; `declared create` replaces `pin bind`.
- ADR 0018 — session lineage.
- ADR 0029 — aliases and lockstep mux rename.
- ADR 0057 — session pins; superseded except for history.
- ADR 0084 — first-class pin nodes; amended.
- ADR 0090 — pin-store registry; superseded.
- ADR 0097 — the launch-spec form the slot edit form reuses.
- ADR 0140 — history; slots target its records.
- ADR 0142 — rebuild plans; what a slot rebuilds into.
- ADR 0143 — the mutation envelope; `pins.toml` is category 1.
- ADR 0144 — the Pinned view.
