# ADR 0027: Workspace Detection And Provider Precedence

## Status

Accepted

## Context

Conspectus needs workspace grouping for multi-repo operator views, but an
ordinary parent directory containing code should not automatically become a
semantic workspace. ADR 0026 also makes workspaces an overlay context:
sessions can belong to both a workspace and an underlying checkout. That only
works if workspace inference is conservative enough to avoid inventing noisy
or misleading parent groups.

Current behavior has two workspace sources:

- Atelier discovery claims a workspace root when `atelier.toml` is present.
- Generic discovery infers a workspace when an explicit scan root has at least
  two immediate git repo children.

Agent-deck and similar tools will add provider-specific workspace roots where
the workspace root may not itself be a git checkout, but its immediate members
resolve to checkouts elsewhere on disk. Generic inference can see some of that
shape, but it cannot know provider identity, intended member roles, labels, or
local state location.

## Decision

Workspace detection is conservative and source-ordered:

1. User-declared workspace links and ignores win over all discovered evidence.
2. Provider-specific workspace metadata wins over generic layout inference for
   the same canonical root.
3. Generic workspace inference is allowed only for explicit scan roots, not for
   recursive discovery under arbitrary parents.

A generic workspace is inferred only when an explicit scan root:

- is not itself a git checkout
- is not claimed by provider-specific workspace metadata
- has at least two immediate child entries that are directories or symlinks and
  probe as git checkouts

Generic inference does not recurse into nested directories to find enough
repos. Nested repos remain repo/checkout evidence until a provider or declared
link supplies workspace intent.

When multiple providers claim the same canonical root, Conspectus merges the
workspace identity for v1 and keeps all candidate evidence, but projections
prefer the strongest provider in this order:

1. local declared state
2. global declared state
3. provider-specific discovered metadata
4. generic convention inference
5. cached evidence

Provider-specific adapters should emit membership links with enough metadata
to preserve the workspace-visible logical path, canonical checkout path when
known, provider member name, and whether the member was a symlink, directory,
or provider-declared unresolved path.

## Consequences

- A directory with one repo remains a repo/checkout view, not a workspace.
- A directory with two immediate repos can be a useful generic workspace when
  the operator explicitly scans it.
- Atelier, agent-deck, and future workspace providers can provide richer
  identity and membership without competing with generic inference for the
  same root.
- Generic inference remains cheap and predictable because it only inspects
  immediate children of explicit roots.
- Workspace membership can preserve both logical and canonical paths, which
  unblocks H-CHECKOUT-004 without making symlink targets the display identity.

## Alternatives Considered

- **Infer workspaces from any common parent of discovered sessions.** Rejected
  because it would make incidental filesystem layout look intentional and
  would group unrelated repos under broad directories such as `~/src`.
- **Let generic inference and provider adapters both emit workspace links for
  the same root.** Rejected because it creates duplicate or conflicting
  membership evidence without adding useful operator information.
- **Require provider metadata for all workspaces.** Rejected because loose
  multi-repo directories are common and useful enough to support when the user
  explicitly scans them.
- **Recurse through scan roots until two repos are found.** Rejected because it
  makes default discovery cost and grouping semantics much less predictable.

## Open Questions Answered

- The generic threshold is two or more immediate git checkout children under an
  explicit scan root.
- Provider-specific metadata takes precedence over generic inference at the
  same canonical root.
- Nested workspaces and nested repos are not collapsed by generic inference.
  They need provider metadata or declared links to become workspace structure.
- Symlinked workspace members should retain both logical member path and
  canonical checkout identity in source metadata.
