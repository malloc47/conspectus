# ADR 0014: Declared Link Storage Schema

## Status

Accepted

## Context

Phase 05 introduces durable user-authored relationships. These
declarations must guide resolution without deleting discovered
evidence. They also need to support ignored and overridden candidates,
because the graph can contain noisy or ambiguous evidence that users
may want to demote while still keeping it inspectable.

ADR 0012 already defines the config-file locations and precedence:

1. project-local `.conspectus.toml`
2. user-level `$XDG_CONFIG_HOME/conspectus/config.toml` or
   `$HOME/.config/conspectus/config.toml`

`CLAUDE.md` requires project-rooted user intent to live near the
relevant repo or workspace, while rebuildable caches must stay outside
project trees. Read-only commands (`graph` and `session`) must not
create or mutate config files.

The schema needs to be:

- readable and hand-editable TOML
- provider-neutral where possible
- precise enough to reconstruct `GraphLink` candidates
- stable across future changes to Rust `Display` implementations
- forward-compatible with future sections and fields

## Decision

Declared relationship state lives in a `declared` table in either the
project-local or user-level config file.

```toml
[declared]
schema_version = 1

[[declared.links]]
id = "codex-alpha-to-editor"
relation = "linked_to_mux"
state = "active"
source = { type = "agent_session", harness_key = "codex", state_scope = "/home/me/.codex", session_key = "alpha" }
target = { type = "mux_session", native_id = "tmux:editor" }

[[declared.links]]
id = "ignore-stale-mux-candidate"
relation = "linked_to_mux"
state = "ignored"
reason = "stale tmux session"
source = { type = "agent_session", harness_key = "codex", state_scope = "/home/me/.codex", session_key = "alpha" }
target = { type = "mux_session", native_id = "tmux:old-editor" }

[[declared.links]]
id = "old-choice"
relation = "linked_to_mux"
state = "overridden"
overridden_by = "codex-alpha-to-editor"
source = { type = "agent_session", harness_key = "codex", state_scope = "/home/me/.codex", session_key = "alpha" }
target = { type = "mux_session", native_id = "tmux:old-editor" }
```

Each declared link has:

- `id`: stable user-facing identifier, unique within the store.
- `relation`: a `RelationKind` serialized as snake_case.
- `state`: `active`, `ignored`, or `overridden`.
- `source`: a typed endpoint.
- `target`: a typed endpoint.
- `reason`: optional explanatory text for ignored or active entries.
- `overridden_by`: required for `overridden` entries, naming the
  replacing declared link id in the same effective config set.
- `label`: optional display label for future table or TUI surfaces.

The file location determines declared provenance:

- `.conspectus.toml` entries become `LocalDeclared` evidence.
- user-level config entries become `GlobalDeclared` evidence.

Local declarations have higher resolver precedence than global
declarations. Both beat discovered, convention-derived, and cached
evidence. Ignored and overridden links stay visible in
`candidate_links` but are excluded from preferred resolution, matching
the existing `LinkState` behavior.

### Endpoint Encoding

Endpoints are typed inline TOML tables. They intentionally mirror the
stable fields of graph node IDs rather than the `Display` strings.

```toml
{ type = "repo", common_dir = "/work/repo/.git" }
{ type = "checkout", repo_common_dir = "/work/repo/.git", root = "/work/repo" }
{ type = "workspace", root = "/work" }
{ type = "agent_session", harness_key = "codex", state_scope = "/home/me/.codex", session_key = "alpha" }
{ type = "mux_session", native_id = "tmux:editor" }
{ type = "branch", repo_common_dir = "/work/repo/.git", refname = "refs/heads/main" }
{ type = "fork", provider_source_key = "atelier:alpha" }
{ type = "forge_pr", provider = "github", host = "github.com", owner = "octo", repo = "repo", number = 7 }
```

When an endpoint does not exist in the current graph, discovery should
preserve it as unresolved endpoint evidence rather than failing or
creating placeholder nodes. When the endpoint is present, it may resolve
to the matching `NodeId`.

### Write Ownership

Read-only commands never create, update, or remove declared-link state.
Only explicit mutation commands may write these files.

Write commands choose the nearest appropriate store by default:

- declarations rooted in a discovered repo, workspace, worktree, branch,
  fork, or forge PR go to the nearest project-local `.conspectus.toml`
- orphan agent-session and mux-only declarations go to user-level config
- generated caches and indexes are never the durable source of
  user-authored intent

CLI commands may expose an explicit store override later, but the
default behavior follows those ownership rules.

### Compatibility

`schema_version = 1` applies only to the `declared` table. Existing
files with only `[session]` remain valid. Unknown fields inside
declared-link entries are ignored on read and preserved when practical
by write helpers. Unknown `schema_version` values produce diagnostics
and suppress declared-link loading from that table rather than failing
the whole config load.

Malformed declared-link entries produce diagnostics and are skipped.
Valid sibling entries in the same file still load. Missing
`declared.links` means there are no declarations.

## Consequences

- Declared links can be converted into the existing `GraphLink` model
  without changing resolver semantics.
- The schema remains readable enough for hand-authored project config.
- Conspectus does not depend on parsing `NodeId` display strings.
- Local-vs-global precedence stays a property of config location, not
  a mutable field users can accidentally mis-set.
- The first write implementation must preserve unrelated TOML sections
  so `[session]` and future config are not destroyed by link commands.

## Alternatives Considered

- **Store graph ID display strings directly.** Rejected because display
  strings are useful diagnostics, not a committed parsing format.
- **Store full JSON-serialized `NodeId` objects in TOML.** Rejected
  because it is awkward to hand-edit and makes common declarations
  noisy.
- **Use one table per endpoint type.** Rejected because each
  relationship has exactly two endpoints; typed inline tables keep the
  link self-contained.
- **Store declarations only in a global database or cache.** Rejected
  because project-rooted user intent should live near the project and
  remain reviewable.
- **Physically delete ignored evidence from graph output.** Rejected
  because ADR 0002 makes `GraphLink` authoritative for evidence and
  diagnostics. Ignoring affects preferred resolution, not evidence
  preservation.

## Open Questions Answered

- Declared state uses TOML in existing config locations.
- The `declared` table has its own schema version.
- Endpoint identity is stored as typed fields, not opaque display text.
- File location determines local/global declared provenance.
- Read-only commands never write config files.
- Ignored and overridden entries stay visible as candidate evidence.
