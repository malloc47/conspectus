# ADR 0001: Node Identity And Stable IDs

## Status

Accepted

## Context

Conspectus builds a graph from entities with very different lifetimes:
ephemeral agent sessions and fork state, lifecycled forge PRs, and long-lived
git repositories. The graph must be useful in a read-only discovery pass, avoid
assuming dense links, and stay provider-neutral even when individual adapters
use provider-specific evidence.

Stable identity is needed before Conspectus can safely persist declared links,
cache discovery results, or expose a machine-readable graph to other tools. At
the same time, v1 should not require Conspectus to write IDs into every repo or
workspace just to discover local state.

Git provides useful local identity surfaces:

- A git repository with linked worktrees has a shared common metadata directory.
- Each worktree has its own root and per-worktree metadata.
- Branches are refs inside a repo and can move between commits and worktrees.

Agent harnesses do not provide a uniform global session identity. Some harnesses
emit UUID-like session IDs, some use timestamp-derived IDs, and some expose only
a synthetic or path-scoped session placeholder.

## Decision

Use structured, provider-aware local identities for v1. Preserve
provider/source metadata as aliases and evidence. Introduce optional durable
local IDs only when Conspectus writes declared state.

Entity identity rules:

- `Repo`: identify by canonical git common dir for local discovery. Store
  remotes, source paths, default remote, and normalized remote URLs as
  attributes or aliases, not as the primary key.
- `Worktree`: identify by `(repo_id, canonical worktree root)`. Use git
  per-worktree metadata path as source evidence when available. Do not include
  the current branch in worktree identity.
- `Workspace`: identify provider-backed workspaces by provider plus canonical
  root. Identify generic inferred workspaces by canonical root. If Conspectus
  later writes `.conspectus.toml`, it may add an optional local `workspace.id`
  to survive path moves.
- `AgentSession`: identify by `(harness_key, state root or scope, native
  session id or source path fallback)`. Do not assume harness-native session IDs
  are globally unique.
- `Branch`: identify by `(repo_id, refname)`. Store current commit, upstream,
  remote, and checked-out worktrees as attributes or links.
- `ForgePr`: identify by `(forge_provider, host, owner, repo, number)`. Store
  head/base refs, state, URLs, and provider-specific fields as attributes or
  source metadata.

## Consequences

- Read-only discovery can produce stable local graph nodes without writing
  Conspectus IDs into every discovered repo or workspace.
- Declared links can refer to deterministic structured IDs, while later local
  `.conspectus.toml` files can add durable aliases for path-move resilience.
- Repo identity remains robust for local worktree workflows because linked
  worktrees share the same git common dir.
- Remote URL changes, repo moves, or workspace moves may create apparent new
  nodes unless Conspectus has declared state, cache aliases, or a future local
  durable ID to reconcile them.
- Composite agent-session IDs are less compact than harness-native IDs, but
  they handle harness state relocation, path-scoped sessions, and synthetic
  session IDs.
- Branch identity is stable across commits but intentionally does not identify a
  single line of history forever; branch reuse and historical PR ambiguity
  remain relationship/provenance concerns.

## Alternatives Considered

- Use remote URL as primary `Repo` identity. Rejected because local-only repos,
  mirrors, forks, multiple remotes, and renamed hosts make remote identity too
  ambiguous for local discovery.
- Generate durable IDs for every discovered repo and workspace immediately.
  Rejected for v1 because it conflicts with read-only-first discovery and would
  dirty project trees too eagerly.
- Use harness-native session IDs directly. Rejected because harness IDs are not
  guaranteed globally unique and some harnesses expose path-scoped or synthetic
  session identifiers.
- Use branch name or current commit as part of `Worktree` identity. Rejected
  because branches and commits change while the checkout path remains the thing
  users navigate.

## Open Questions Answered

- Harness-native session IDs should not be assumed globally unique.
- Worktree identity should be path-based plus git metadata evidence, not
  branch-based.
- Workspace identity should be provider/root-based at first, with optional
  durable local IDs only when Conspectus writes declared state.
