# ADR 0026: Checkout Context Model

## Status

Accepted

## Context

The original model used `Worktree` for the concrete path where a user or
agent edits files. That was convenient for git, but the word over-indexes
on the linked-worktree feature and no longer matches the local workflows
Conspectus must explain.

Observed and expected workflows include:

- working directly in an ordinary `git clone` checkout
- working in a linked git worktree created from a non-bare clone
- working in a linked git worktree created from a bare clone
- working in a workspace that symlinks to ordinary clones elsewhere on disk
- working in a workspace with worktrees linked to repos elsewhere on disk

The sessions view exposed the mismatch. A discovery pass can find many
agent sessions with stable `cwd` values, but only sessions whose `cwd`
exactly matches an already discovered git worktree root get grouped under
that context. Sessions rooted in other useful checkouts fall into
`Ungrouped`, even when they share the same repo, workspace, or orchestrator
context. Separately, workspace-oriented tools such as agent-deck may expose a
persistent workspace root while each member path still resolves to an
ordinary clone or linked worktree. In those cases users reasonably expect the
same agent session to be visible both under the workspace and under the
individual checkout it is acting on.

ADR 0001's identity rule for `Worktree` remains structurally sound: identify
the edit surface by repo identity plus canonical checkout root, and keep
current branch out of identity. What needs to change is the product and graph
language around the entity, the discovery surfaces that create it, and the
projection rules that group sessions.

## Decision

Adopt **`Checkout`** as the provider-neutral model term for a concrete
working tree or editable repository view. A checkout may be an ordinary clone
checkout, a linked git worktree, a linked worktree whose common dir belongs to
a bare repository, or a workspace member reached through a symlink.
Implementation now uses `Checkout` type names and JSON fields; source adapters
may still preserve provider-native `worktree` strings when they describe
actual git or Atelier formats. The north-star model and new work should use
`Checkout`.

Checkout identity for local git discovery is:

- `Repo`: canonical git common dir, as in ADR 0001.
- `Checkout`: `(repo_id, canonical checkout root)`.

Discovery must preserve enough path evidence to explain how a session reached
the checkout:

- canonical checkout root
- session `cwd` as recorded by the harness
- workspace-local logical path when the checkout is reached through a symlink
  or provider member path
- git common dir and per-checkout git dir when available
- checkout kind/source metadata, such as `plain_clone`, `linked_worktree`,
  `bare_repo_worktree`, `workspace_symlink_member`, or `unknown_git_checkout`

Agent and mux sessions can be associated with multiple context nodes. A
session rooted in a workspace member should link to both the `Workspace` and
the underlying `Checkout` by default. Projections may therefore render the
same session in more than one grouping context when that reflects the graph
instead of treating grouping as a single-parent tree.

Default grouping precedence for context-oriented views is:

1. workspace context
2. checkout derived from a bare-repository worktree
3. linked git worktree
4. ordinary clone checkout
5. sparse cwd/path fallback

`Ungrouped` should be reserved for sessions without usable path or context
evidence, not for sessions whose `cwd` points at a git checkout outside the
current scan root.

Workspace display should support an include/exclude control:

- **include workspaces**: default; show workspace groupings and checkout
  groupings, allowing duplicate session representation when useful
- **exclude workspaces**: hide workspace groupings and show checkout/repo/path
  contexts only
- **workspace only** may be added later if operators want an explicitly
  workspace-centric projection

Discovery must probe distinct agent-session and mux-session `cwd` paths
read-only, even when those paths are outside the launch cwd or configured scan
roots. Those probes should backfill `Repo`, `Checkout`, and `Branch` nodes
and emit candidate links between sessions, workspaces, repos, checkouts, and
branches. Configured scan roots and provider metadata still matter for
broader discovery, but an observed session cwd is direct evidence worth
probing.

## Consequences

- Plain clones, linked worktrees, bare-repo-derived worktrees, and symlinked
  workspace members all have a single conceptual home in the graph.
- The current sessions view's `Ungrouped` bucket should shrink sharply once
  cwd probing creates checkout nodes for observed session paths.
- Workspaces become an overlay context rather than a replacement for repo and
  checkout links. This matches agent-deck / atelier style workflows where the
  workspace is persistent and each member remains a real checkout elsewhere.
- Some projections must support multi-home rows or explicit deduplication
  modes. A strict tree remains a view choice, not a graph invariant.
- Graph JSON and model names now use checkout terminology. Conspectus is not
  in active external use, so user-facing aliases should not be added just to
  preserve legacy `worktree` spelling.
- Fork relation kinds from ADR 0004 use checkout terminology, such as
  `created_checkout` and `referenced_checkout`.

## Alternatives Considered

- **Keep `Worktree` as the only term and document that it also means plain
  clone checkout.** Rejected because the overloaded git term already caused
  incorrect expectations and makes workspace/member modeling harder to reason
  about.
- **Group sessions only by workspace when a workspace can be inferred.**
  Rejected because users still need to see the individual repo or checkout
  affected by a session, and many sessions have no workspace context.
- **Manufacture generic workspaces for every common parent directory.**
  Rejected because it turns incidental filesystem layout into product
  semantics and obscures standalone repo workflows.
- **Force each session into exactly one grouping parent.** Rejected because a
  workspace-member session truthfully belongs to both the workspace and the
  underlying checkout. Single-parent output can be offered as a projection
  mode, but it should not constrain the graph.

## Open Questions Answered

- A vanilla clone checkout is a first-class checkout, not merely a repo.
- A linked git worktree is one checkout kind, not the whole entity category.
- Bare-repo-derived worktrees use the same checkout identity rule; the bare
  repo detail is source metadata and grouping precedence.
- Symlinked workspace members should preserve both workspace-local logical
  path and canonical checkout root.
- Workspace contexts are included by default, with an exclude toggle for users
  who do not use workspace-oriented tools.
