# ADR 0003: Polymorphic Fork Node

## Status

Accepted

## Context

The initial Conspectus design split fork modeling into `ContextFork`,
`SessionFork`, and `ForkGroup` nodes. That split is semantically precise, but it
risks over-normalizing a graph model whose main job is to capture sparse,
provider-neutral provenance links.

Fork-like behavior is not uniform across providers or workflows. A fork may
create isolated worktrees, reference existing worktrees, start a new agent
session, continue from an existing agent session, group multiple harness
sessions, or exist only as metadata. Atelier alone has worktree, selected, and
research fork modes, plus per-harness session records with native, approximate,
unsupported, or fresh-session semantics.

Conspectus already uses `GraphLink` candidates and a nontrivial resolver layer.
That architecture can represent the effects of a fork through relation kinds
without requiring separate fork node classes.

## Decision

Use one provider-neutral polymorphic `Fork` node instead of separate
`ContextFork`, `SessionFork`, and `ForkGroup` nodes.

`Fork` represents a fork-like provenance concept or operation. Its meaning is
expressed by attributes, provider/source metadata, and links to affected graph
entities.

Recommended `Fork` attributes include:

- provider
- provider source key
- kind or capabilities, such as context, session, combined, metadata-only,
  creates-worktree, references-worktree, read-only, approximate, unsupported, or
  fresh-session
- scope, such as workspace, repo, session, or global
- created timestamp when available
- human-facing name when available

Relation kinds should express what the fork affected:

- `forks_workspace`
- `forks_repo`
- `created_worktree`
- `referenced_worktree`
- `parent_session`
- `child_session`
- `created_branch`
- `associated_branch`
- `parent_fork`
- `rooted_at_path`
- provider-specific source/evidence links where needed

For Atelier:

- One `.atelier/forks/index.toml` `ForkEntry` maps to one `Fork` node.
- Worktree-mode repo entries produce `created_worktree` and branch association
  links.
- Selected-mode repo entries produce `created_worktree` links for forked repos
  and `referenced_worktree` links for symlinked parent repos.
- Research-mode fork entries produce a `Fork` node with root/provider metadata
  and reference links, but no created worktrees.
- Each `ForkHarnessEntry` contributes `parent_session` and/or `child_session`
  link candidates when source or fork session IDs are present.
- Atelier-specific fields such as mode, state, read-only flag, sandbox
  override, and harness capability are preserved as source metadata unless a
  later data-model decision promotes them to provider-neutral attributes.

## Consequences

- The fork model is simpler for consumers: there is one fork node type to
  render, filter, and link.
- Context lineage and session lineage remain distinguishable through relation
  kinds instead of separate node classes.
- Sparse and partial cases are easier to represent. Research forks,
  selected-mode symlinks, unsupported harness session forks, and fresh sessions
  can all exist as one fork node with different links and evidence.
- A single provider operation that affects several repos and harnesses remains
  naturally grouped as one `Fork`.
- The resolver and projection layers must understand fork relation kinds well
  enough to render context lineage and session lineage separately when needed.
- The design gives up some compile-time precision that separate `ContextFork`
  and `SessionFork` types would have provided, but this is offset by a simpler
  graph shape and better fit with `GraphLink` resolution.

## Alternatives Considered

- Keep separate `ContextFork`, `SessionFork`, and `ForkGroup` nodes. Rejected
  because it overfits the model before enough providers exist and creates extra
  node types for sparse or partial fork-like evidence.
- Map each provider fork only to a `ForkGroup`. Rejected because it hides the
  actual affected entities unless all context and session effects are packed
  into provider metadata.
- Model forks only as links with no fork node. Rejected because users need a
  visible provenance concept for fork names, roots, timestamps, lineage, and
  provider operations.

## Open Questions Answered

- One Atelier `ForkEntry` should become one `Fork` node.
- A single Atelier fork can link to multiple repos, worktrees, branches, and
  agent sessions without creating separate context/session fork nodes.
- Atelier repo membership states should map to relation kinds such as
  `created_worktree` and `referenced_worktree`.
- Research forks are represented as fork nodes with metadata and reference
  links, even when they create no worktrees.
- Atelier-specific fork fields remain source metadata unless later promoted to
  provider-neutral attributes.
