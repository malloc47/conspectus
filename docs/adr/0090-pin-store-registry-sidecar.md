# ADR 0090: Pin-Store Registry Sidecar

## Status

Accepted

## Context

Session pins (ADR 0057) live in `[[pins.entries]]` sections of
project `.conspectus.toml` files and the user-scope config. Pin
discovery (`src/discovery/pins.rs`) locates project pin stores by
walking up from a search-root set built from two sources:

1. the active scan roots (`DiscoveryContext::roots()`), and
2. the cwds of already-discovered nodes (repos, checkouts,
   workspaces, agent sessions, mux sessions, runtime processes).

A pin store is only re-read when one of those paths leads
`ConfigLoader::locate_project_config` to it. That leaves a gap:

- The operator creates a pin for a repo that is **not** part of the
  current scan root.
- The pin's mux session isn't running yet (it was just created, not
  launched), so no discovered node has a cwd inside that repo.

With neither a scan root nor a node cwd pointing into the repo, the
store is never located, and the freshly-created pin vanishes on the
next discovery cycle. This is a concrete, reported operator bug
(H-PIN-ROOT-001): "creating a new pin outside of the cwd doesn't
appear when the repo it is registered in is not part of the search
root."

Discovery-time re-registration cannot fix this on its own — it is
chicken-and-egg. Discovery only reads stores it can already locate,
so a store outside every search root is never read, and therefore
never available to register at read time. The store location has to
be captured at the moment Conspectus **writes** the pin, when the
path is known explicitly.

## Decision

Conspectus maintains a **pin-store registry sidecar**: a rebuildable
cache that records the on-disk locations of project pin stores so
they stay discoverable regardless of the active scan root.

### Shape and location

The registry lives at
`$XDG_STATE_HOME/conspectus/pin-stores.json` (with the standard
`$HOME/.local/state/conspectus/` fallback), a sibling of the TUI
state file. On disk:

```json
{
  "schema_version": 1,
  "stores": [
    "/home/you/src/repoA/.conspectus.toml",
    "/home/you/work/repoB/.conspectus.toml"
  ]
}
```

Reads parse through a struct with a flattened `extra` map so unknown
fields round-trip unchanged across schema bumps. Writes are
best-effort, idempotent, skip-on-unchanged, and atomic (tempfile +
rename via the shared `write_atomic` helper).

### Write path

When Conspectus writes a **project** pin store — the CLI `pin
create` / `pin adopt` paths and the TUI pin-create action, all of
which upsert into a project `.conspectus.toml` — it records that
store path in the registry. Registration is:

- **Project-only.** The user-scope store (`config.toml`) is a no-op
  because discovery always consults it directly.
- **Absolute-only.** Non-absolute paths are ignored so the on-disk
  list stays unambiguous.
- **Self-healing.** Each write prunes recorded paths that no longer
  resolve to a file, so deleted repos fall out of the registry.
- **Non-fatal.** A failed registry write never fails the pin write;
  the pin's authoritative intent already landed in the
  `.conspectus.toml`.

### Read path

Each discovery cycle reads the registry fresh (so a pin created
during a live TUI session appears on the next refresh) and folds the
recorded store paths into the pin loader's search set, deduplicated
against the scan-root/node-derived paths and filtered to stores that
still exist. The registry resolver is carried on
`LocalDiscoveryConfig::pin_store_registry`; `None` disables it for
headless fixtures and tests that don't want state-home I/O.

### Envelope placement

This is a **category 2** write under ADR 0087 (rebuildable
observation sidecars under `$XDG_STATE_HOME/conspectus/`). It
qualifies on every count:

- **Rebuildable.** Deleting `pin-stores.json` only means out-of-root
  pins stop showing until they are written again. Nothing depends on
  it to function.
- **No operator intent Conspectus authored.** The authoritative pin
  intent is the `[[pins.entries]]` section the operator wrote; the
  registry only records *where* those stores are, an observation
  Conspectus can re-derive by writing again.
- **No payload.** Store paths are attribution/observation, not
  conversation content (ADR 0086 Tier 3).

No new mutation category is introduced, so ADR 0087 is extended by
reference, not superseded. The write is operator-initiated (it only
fires as a side effect of an operator's pin-create gesture); the
background daemon never registers stores, consistent with ADR 0087
prohibition 5 — it reads the registry, like it reads `graph.bin`,
but does not write it.

## Consequences

- A pin created in any repo, in or out of the scan root, stays
  visible on subsequent runs — the reported bug is fixed.
- The registry grows by one entry per distinct project store the
  operator ever pins into, self-pruning as repos disappear. It is
  bounded by the number of repos the operator pins, not by activity.
- `conspectus graph` / `table` and other read surfaces stay
  read-only for pins: they consume the registry but never write it.
  Only the pin-create write paths register.
- Existing pins authored before this feature are captured the next
  time their store is written to (re-saving any pin in that store),
  or whenever a scan root / running session brings the store into
  discovery range as before. There is no migration step.

## Alternatives Considered

**Treat a pin's own `cwd` as an extra project-config search root.**
Rejected as insufficient. It is circular for a cold run: we can't
read the store to learn its cwd if we never locate the store. It
would only help within a session where the store had already been
found by other means.

**Explicit user-config list of pin-store directories.** Rejected as
too manual. It works, but it pushes the bookkeeping onto the
operator for every out-of-root repo, which is exactly the friction
the pin feature exists to remove.

**Register stores during discovery reads.** Rejected as
chicken-and-egg (see Context) — a store outside every search root is
never read, so read-time registration can never capture the case the
bug is about. Write-time registration is the only point where the
out-of-root store path is known.

**Store the pin `cwd` instead of the store path.** Rejected. The
loader needs the `.conspectus.toml` path to read; storing the cwd
would just re-run `locate_project_config` per entry for no benefit,
and the store path is the stable, directly-usable key.

## Open Questions Answered

- **Does the daemon write the registry?** No. Only operator-initiated
  pin-create write paths register. The daemon reads it, consistent
  with ADR 0087 prohibition 5.
- **What happens when a recorded repo is deleted?** The next
  registry write prunes it, and reads filter out stores whose file no
  longer exists, so a stale entry is inert in the meantime.
- **Are user-scope pins recorded?** No. Discovery consults the
  user-scope store directly, so only project stores are registered.

## Related ADRs

- ADR 0057 (session pins) — the `[[pins.entries]]` store this
  registry indexes.
- ADR 0084 (first-class pin nodes) — the pin nodes that depend on
  the store being discoverable.
- ADR 0086 (payload privacy tenet) — store paths are Tier 3
  observation, never payload.
- ADR 0087 (mutation envelope) — this registry lands in category 2
  (rebuildable observation sidecars); this ADR extends it by
  reference.
