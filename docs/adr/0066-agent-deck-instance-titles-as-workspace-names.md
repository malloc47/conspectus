# ADR 0066: Agent-Deck Instance Titles As Workspace Display Names

## Status

Accepted

## Context

ADR 0060 established the agent-deck multi-repo workspace adapter,
which emits one `Workspace` node per
`~/.agent-deck/multi-repo-worktrees/<folder>/` directory with
`name = Some(<folder>)`. The folder name is opaque to the operator:

- Agent-deck-generated folders look like `345062f6` (8-hex
  conductor id) or `feature-nix-config-c7cf4c65` (slug + 8-hex).
- Operators rename their sessions inside agent-deck — agent-deck
  stores those names in `~/.agent-deck/profiles/<profile>/state.db`
  in the `instances` table (`id`, `title`, `title_locked`, …).
- The folder name is never updated to reflect the rename, so the
  Sessions/Workspace view's headers showed the raw folder
  instead of the operator-meaningful title (`nix-config`,
  `nixos-config-ssh-copy-paste`, …).

The data exists, the linkage is straightforward (the folder's
trailing hyphen-segment matches the instance id's first
hyphen-segment), and the title is exactly what the operator typed
when renaming the session inside agent-deck.

## Decision

Extend the `agent-deck` discovery adapter to read instance titles
from agent-deck's profile databases and use them as the
`WorkspaceNode.name` whenever a matching instance exists.

Rules:

1. **Profile enumeration.** Walk every immediate subdirectory of
   `<agent-deck-root>/../profiles/` and open each one's
   `state.db` read-only. Profiles are SQLite databases agent-deck
   manages; conspectus opens with `SQLITE_OPEN_READ_ONLY` so we
   never compete with agent-deck for the write lock.
2. **Map build.** From each profile's `instances` table, pull
   `(id, title)` rows and build a single
   `BTreeMap<String, String>` keyed by the **id prefix** — the
   substring before the first `-` in `id`, which is the 8-hex
   conductor id agent-deck shares with folder names. First profile
   to register a prefix wins; the directory walk is sorted so the
   outcome is deterministic across runs.
3. **Folder → prefix.** For each workspace folder name, take the
   substring after the **last** `-` (or the whole name if no
   `-`). That gives the 8-hex suffix that matches an instance's
   id prefix.
4. **Substitute.** When the map yields a non-empty title, set
   `WorkspaceNode.name = Some(<title>)`. Otherwise keep
   `Some(<folder-basename>)` — current behavior.
5. **Tolerance.** A missing profiles directory, an unreadable
   `state.db`, or an absent `instances` table are non-fatal: the
   adapter logs nothing extra and falls back to the folder name.
   Discovery must not fail because agent-deck isn't installed,
   isn't using a profile-shaped layout, or has the DB locked at
   exactly the wrong moment.

The adapter's existing symlink-layout contract is unchanged — the
title lookup is a metadata enrichment, not a re-shape of the
workspace graph. Membership candidate links still come from the
symlinks; only `WorkspaceNode.name` reads from `state.db`.

## Consequences

- **Operator-meaningful workspace headers.** The Sessions/Graph
  and Sessions/Workspace views now read `nix-config` instead of
  `feature-nix-config-c7cf4c65`. The `format_workspace_display`
  helper (ADR 0062) picks up the new name automatically — no
  formatter change.
- **Live-state coupling, narrowly scoped.** The agent-deck
  adapter now reads a live SQLite database that agent-deck
  writes to concurrently. The read-only open and the prefix-only
  field selection keep the contact surface minimal; if
  agent-deck changes `instances`'s schema, the column probe
  fails and we fall through to folder names — no crash.
- **Multi-profile support out of the box.** Operators with
  multiple profiles get titles from every profile merged. First-
  hit-wins is the deterministic tiebreaker; in practice agent-deck
  ids don't collide across profiles so the rule rarely matters.
- **No data-model change.** `WorkspaceNode.name` was already
  `Option<String>`. The graph schema, candidate-link shapes, and
  resolver behavior are untouched.
- **`title_locked` is ignored on read.** Whether the title was
  manually renamed or auto-derived, we use it. The distinction
  matters only to agent-deck's own UX (don't auto-overwrite a
  locked title); for conspectus, the title field is canonically
  whatever agent-deck currently considers the display string.

## Alternatives Considered

### A. Read titles only from the `default` profile

Simplest possible scope: hardcode `profiles/default/state.db`.
Rejected on the operator's call — they want every profile
covered.

### B. Add a separate `display_name` field on `WorkspaceNode`

Keep `name = folder` and add `display_name: Option<String>` for
the title. Stricter separation between identity and display, but
no caller distinguishes those today; the only `name` consumer is
`format_workspace_display`. Adds a schema-level field for one
provider's enrichment. Rejected — single field is simpler.

### C. Project-path-based matching

Match agent-deck instances to conspectus workspaces via
`instances.project_path` (the per-member subdir). Requires
inferring the workspace root from the project path's parent and
breaks when the workspace contains only one member. Rejected in
favor of the direct id-prefix ↔ folder-suffix match, which is
the same key agent-deck uses internally.

### D. Watch state.db for changes

Subscribe to filesystem events on the SQLite file so renames
propagate without a re-discovery cycle. Out of scope — the
Sessions/Graph view already refreshes periodically, so a rename
inside agent-deck surfaces on the next refresh. Revisit if a
push-style update becomes needed.

## Open Questions

- Should the adapter eventually consume agent-deck's
  `groups.name` (group display label) for a higher-level
  grouping inside Sessions/Workspace? Out of scope; the current
  data only has a single `my-sessions` group on the operator's
  machine, so there's no real surface to design against yet.
