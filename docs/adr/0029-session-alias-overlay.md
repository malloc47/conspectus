# ADR 0029: Session Alias Overlay

## Status

Accepted

## Context

Today every name that surfaces for an agent session originates with the harness:
opencode populates its `title` column, claude-code carries an auto-generated
`summary` from transcript compaction, codex and aider populate nothing. The
field is exposed as `AgentSessionNode.title: Option<String>` in
`src/model/mod.rs`, rendered in the `title` column of `conspectus table
sessions`, in `conspectus node show`, and in the TUI sessions row tree and
detail panel.

Operators have asked for the ability to choose their own name for a session
so the TUI and CLI surface something memorable instead of `harness:<short-id>`
or a stale compaction summary. The naming request also extends to tmux
sessions, both independently and in lockstep with the agent-session rename
when one is linked.

Conspectus has been almost entirely read-only to date. The only existing write
surface is Phase 5 declared-link CRUD (ADR 0014), which operates on
*relationships* between two endpoints, not on attributes of a single node.
Adding a "session alias" surface needs an explicit storage decision so it
neither pollutes the declared-link pipeline nor invents a parallel persistence
mechanism with its own conventions.

Several adapter-specific complications constrain the decision:

- Claude Code's `summary` lives at the head of an actively-appended JSONL
  transcript. Writing to that file would race the live harness process and
  has no documented mutation API.
- Codex and aider have no harness-native title field at all.
- Opencode's sqlite `title` column is writable, but writing to it during an
  active session would race the opencode process.

`MuxSessionId = native_id` (`src/model/mod.rs:36`, `297-299`). For tmux the
`native_id` *is* the session name, so renaming a tmux session changes the
node's `NodeId`. Any alias keyed on the old id would be orphaned.

ADR 0028 introduced hook-sidecar evidence that can reattribute a tmux pane
between agent sessions post-`/resume`. An alias surface must not silently
follow pane attribution when the underlying agent-session identity changes.
ADR 0028 stores hook observations behind `conspectus hook write` in a SQLite
local-state backend. That does not by itself change alias storage: hook
observations are rebuildable operational state, while aliases are durable
user-authored intent.

## Decision

Conspectus stores user-chosen names as a **Conspectus-owned alias overlay**.
The harness-native title is never mutated. Aliases are loaded as additional
discovery evidence and applied at projection time. The render precedence is
`alias > title > id-suffix`.

### Storage Schema

Aliases live in a new top-level `aliases` table in the existing TOML config
files (`.conspectus.toml` for local, the user-level config for global). They
do **not** extend the `[declared]` table — aliases are node attributes, not
relationships, and shoehorning them into `[[declared.links]]` would require a
synthetic self-relation that pollutes `RelationKind` and the resolver.

```toml
[aliases]
schema_version = 1

[[aliases.entries]]
node = { type = "agent_session", harness_key = "codex", state_scope = "/home/me/.codex", session_key = "alpha" }
display_name = "ingest-refactor"
reason = "optional explanation"
```

Each entry has:

- `node`: a typed endpoint that mirrors ADR 0014's `DeclaredEndpoint`
  encoding. Reusing the same encoding keeps the two write surfaces visually
  consistent and lets a single endpoint codec serve both (per `CSP-083`).
- `display_name`: required non-empty string. An entry with empty
  `display_name` is invalid; alias removal deletes the entry rather than
  storing an empty value.
- `reason`: optional explanatory text. Reserved for future audit surfaces;
  not currently rendered.

`schema_version = 1` applies only to the `aliases` table. The same
forward-compatibility rules as ADR 0014 hold: unknown fields are ignored on
read and preserved on write when practical, malformed entries are skipped
with a diagnostic, and an unknown `schema_version` suppresses alias loading
from that file rather than failing the whole config load.

### Store Selection and Conflict Resolution

The same provenance and precedence rules as declared links apply:

- `.conspectus.toml` entries become `LocalAlias` evidence.
- user-level config entries become `GlobalAlias` evidence.
- Local aliases win over global aliases when both name the same node.
- Writes choose the nearest appropriate store by walking the node's
  associated repo, checkout, or workspace via the existing
  `select_store_for_declaration` helper in `src/declared.rs`. Orphan
  agent-session and mux-only aliases go to the user-level config.

When an operator-set alias matches the harness-native title verbatim, the
alias is still persisted. Operator intent is signal, not noise; a future
cleanup pass must not silently dedupe.

### Projection Precedence

Aliases do **not** add a `display_name` field to `AgentSessionNode` or
`MuxSessionNode`. Doing so would pollute the canonical graph with a UI
concern, force serde-skip gymnastics across every snapshot fixture, and
diverge from the pure-view-model contract established by ADR 0024.

Instead, the alias loader produces a `HashMap<NodeId, String>` carried
alongside `GraphSnapshot`. Projection builders apply the
`alias > title > id-suffix` precedence at render time at four sites:

- `src/output/table.rs` `title` column rendering
- `src/output/node_show.rs` header field
- `src/tui/rows/mod.rs` `AgentSessionRow` label
- `src/tui/detail.rs` header field

A single centralized helper applies the precedence so a future tweak touches
one place.

### Mux Node-Id Stability

Because `MuxSessionId = native_id` and the tmux `native_id` is the session
name, renaming a tmux session changes the node's `NodeId`. Aliases keyed on
mux nodes would orphan on every rename. Therefore:

- The mux alias overlay is **not stored**. Lockstep mux renames mutate the
  tmux native name directly (via the new `TmuxRunner::rename_session` seam);
  the new native name *is* the display.
- Independent mux renames (`conspectus rename mux <id> <name>`) follow the
  same rule: only the tmux native name changes, no alias row is written.
- Aliases exist only for nodes whose ids are stable across the rename
  operation, which today means agent sessions.
- Future mux backends with stable ids decoupled from display names (zellij,
  per `CSP-111`) may make a mux-alias row meaningful; that decision is
  deferred to whichever ADR introduces the backend.

### Mux Lockstep Policy

When the operator renames a muxed agent session, the default behavior is to
also rename the linked mux session via `tmux rename-session`. The CLI exposes
a `--no-mux` flag to decouple. The lockstep rule:

- Lockstep applies only when exactly one `LinkedToMux` candidate resolves to
  the agent session (the resolver's preferred mux). Ambiguous links refuse
  lockstep and require the operator to either resolve ambiguity first (via
  `conspectus declared confirm` or by hand) or pass `--no-mux`.
- Lockstep applies to **agent-session identity**, not pane attribution. If a
  fresh hook-sidecar record (ADR 0028) later reattributes the same pane to a
  different agent session, the alias stays with the original session; the
  pane's new occupant carries its own (possibly absent) alias.

### Live-Session Safety

Because the alias overlay never mutates harness state, renaming a session
is safe whether the session is live or dormant. The TUI surfaces an
informational status-bar advisory when the target is live (mux indicator
`Attached` or `Ambiguous` with fresh hook-sidecar evidence per ADR 0028's
`ACTIVE_TTL_SECONDS`) so the operator understands the alias overlays the
harness title rather than replacing it.

Future write-back to harness state (where the harness supports it, e.g.
opencode sqlite) is intentionally deferred to a follow-on ADR. That ADR will
need to address live-session safety per harness; the present alias-only
decision keeps the door open without committing to it.

## Consequences

- Renaming works uniformly across every harness, including those with no
  writeable title field (claude-code, codex, aider) and those with active
  file ownership concerns.
- The canonical graph stays free of UI concerns. Snapshot fixtures do not
  churn to add an alias field; the alias loader output is a sidecar map
  applied at projection time.
- Aliases hide harness-native titles in the default render. Operators who
  want to see the underlying title need a read-path command (`conspectus
  alias list`, filed as `CSP-238`).
- Mux node-id instability is handled by not storing mux aliases at all.
  Renames mutate native names directly. A future mux backend with stable
  ids would need a sibling ADR before adopting an alias row.
- The first non-relationship write surface establishes a pattern other
  per-node attributes (ignored-flags, ad-hoc notes) could follow. The
  `[aliases]` table name should remain alias-specific; other attribute
  categories should get their own sibling tables rather than being folded
  in.
- Read-only commands (`graph`, `node show`, `table`, `tui` navigation) must
  not mtime-touch or content-modify alias-bearing config files. Mirror of
  ADR 0014's read-only invariant.

## Alternatives Considered

- **Extend the `[declared]` table with a synthetic self-relation.**
  Rejected. Aliases are node attributes, not relationships. A self-relation
  would pollute `RelationKind`, the resolver's candidate-link pipeline, and
  every consumer that walks `[[declared.links]]`.
- **Write back to the harness-native title (`AgentSessionNode.title`).**
  Rejected for v1 because three of four harnesses have no writeable field
  and the fourth (opencode sqlite) requires careful coordination with the
  live harness process. Deferred to a follow-on ADR if operator demand
  emerges.
- **Add `display_name: Option<String>` to `AgentSessionNode` and
  `MuxSessionNode`.** Rejected because it pollutes the canonical graph
  with a UI concern, breaks every existing snapshot fixture, and
  contradicts ADR 0024's pure-view-model contract.
- **Store mux aliases keyed on `(socket_path, original_name)` to survive
  rename.** Rejected because tmux exposes no stable id beyond `native_id`;
  any synthetic compound key Conspectus invents would be fragile under
  socket changes, restarts, or server moves. The decision not to store a
  mux alias at all is simpler and forces the rename to flow through the
  native name where it belongs.
- **Make alias rename a mutation on a hidden cache file.** Rejected
  because aliases are user-authored intent, not a rebuildable observation.
  `CLAUDE.md` requires project-rooted user intent to live near the
  relevant repo or workspace.
- **Store aliases in the same local SQLite database as hook observations.**
  Deferred. ADR 0028 uses SQLite for rebuildable hook state behind
  `conspectus hook write`. Aliases have different durability, review, backup,
  and project/global placement expectations. Sharing implementation machinery
  may be reasonable later, but a shared physical backend must preserve the
  semantic split between evictable observations and durable user intent.

## Open Questions Answered

- Aliases are a Conspectus-owned overlay; harness-native titles are never
  mutated.
- Aliases live in a sibling `[[aliases]]` TOML table, not under
  `[declared]`.
- Mux aliases are not stored; lockstep mux renames mutate the tmux native
  name directly.
- Render precedence is `alias > title > id-suffix`, applied at projection
  time, not on the node struct.
- Lockstep follows agent-session identity, not pane attribution.
- Renaming a live session is safe under the alias-only model; future
  write-back is deferred.
- The SQLite backend for hook observations does not automatically move aliases
  out of the TOML intent layer; that requires a follow-up ADR.
