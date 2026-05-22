# ADR 0004: Fork Context Relation Semantics

## Status

Accepted

## Context

ADR 0003 selects a single polymorphic `Fork` node instead of separate
`ContextFork`, `SessionFork`, and `ForkGroup` nodes. That leaves context fork
edge cases to be represented through relation kinds, attributes, and source
metadata.

Conspectus must handle fork-like workflows that create isolated worktrees,
reference existing worktrees, create or associate branches, expose a metadata
root without creating any checkout, or operate on a standalone repo without a
formal workspace.

The model must preserve the distinction between isolated and referenced
contexts. For example, a selected Atelier fork may create one repo worktree but
symlink another repo back to the parent workspace. Treating both as created
worktrees would misrepresent edit isolation.

## Decision

Represent fork context effects with a small provider-neutral relation-kind set
plus source metadata.

Core relation kinds:

- `forks_workspace`: `Fork -> Workspace`
- `forks_repo`: `Fork -> Repo`
- `created_checkout`: `Fork -> Checkout`
- `referenced_checkout`: `Fork -> Checkout`
- `created_branch`: `Fork -> Branch`
- `associated_branch`: `Fork -> Branch`
- `rooted_at_path`: `Fork -> Path` or equivalent path evidence
- `parent_fork`: `Fork -> Fork`

Relation semantics:

- `created_checkout` means the fork operation or provider created or owns the
  checkout as an isolated fork artifact.
- `referenced_checkout` means the fork context exposes an existing checkout, but
  edits are not isolated from the referenced parent.
- `created_branch` means the provider created or intentionally allocated the
  branch for the fork.
- `associated_branch` means the branch is relevant to the fork by discovery,
  convention, or declaration, but not proven to have been created by it.
- `rooted_at_path` records a fork root even when the fork creates no checkouts.
- `forks_repo` and `forks_workspace` describe intended context scope, not
  necessarily concrete filesystem effects.
- `parent_fork` records fork lineage between fork nodes.

Standalone repo fork-like contexts are allowed. A `Fork` may link directly to a
`Repo`, `Checkout`, or `Branch` without requiring a `Workspace`. Conspectus
should create or infer a `Workspace` only when there is layout, provider, or
declared evidence of coordinated work.

Provider-specific details, such as an Atelier fork mode or symlink path, should
remain source metadata unless they affect provider-neutral graph behavior.

## Consequences

- Worktree, selected, research, metadata-only, and standalone fork-like
  workflows fit one graph shape.
- Selected forks can truthfully distinguish isolated fork worktrees from
  referenced parent worktrees.
- Research forks remain visible as fork nodes with roots and provenance even
  when they create no worktrees or branches.
- Standalone repo workflows do not require manufacturing fake workspace nodes.
- Table views and downstream tools can render isolation and lineage by reading
  relation kinds instead of provider-specific fields.
- The relation-kind set must stay disciplined. New provider details should be
  source metadata by default unless they change graph behavior.

## Alternatives Considered

- Use one generic `fork_affects` relation with attributes. Rejected because
  projections and diagnostics would constantly need to inspect attributes, and
  isolation/reference semantics would be too easy to blur.
- Create separate node types for worktree, selected, and research fork cases.
  Rejected because ADR 0003 chose a polymorphic fork node to avoid
  over-normalizing provider-specific behavior.
- Require all fork-like contexts to belong to a workspace. Rejected because
  Conspectus supports individual repos and loose workspace inference.

## Open Questions Answered

- Research forks are `Fork` nodes with `rooted_at_path` and optional reference
  links, not failed context forks.
- Selected forks use both `created_checkout` and `referenced_checkout` links.
- Created, referenced, associated, and metadata-only fork effects are
  represented with relation kinds plus source metadata.
- Standalone repo fork-like contexts are allowed and do not require an enclosing
  workspace.
