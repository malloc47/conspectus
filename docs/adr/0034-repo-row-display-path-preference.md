# ADR 0034: Repo Row Display Path Preference

## Status

Accepted

## Context

The sessions row builder shows a `Repo` group row above one or more
`Checkout` rows whenever a repo fans out into multiple known checkouts. The
row's bolded label is the last component of a display path, and the dim
secondary text is the full path; both come from a single
`repo_display_path` string per repo.

Until now, `repo_display_path` returned `RepoNode.source_paths.first()` and
only fell back to the git common dir (with `/.git` stripped) when the
`source_paths` vector was empty. That heuristic was written when each
`RepoNode` was assumed to be probed from its canonical checkout, so its
first source path was the canonical clone.

In practice that assumption does not hold once provider-specific
multi-repo checkouts are involved. `merge_fragments` deduplicates nodes by
identity with first-writer-wins (`src/discovery/mod.rs:309`): the first
`RepoNode` for a given `RepoId` keeps its `source_paths`, and subsequent
fragments that probe other checkouts of the same repo are dropped wholesale.
When an agent-deck multi-repo checkout (or any other non-canonical
worktree) is probed before the canonical clone, the `RepoNode` ends up with
`source_paths = [agent_deck_path]` and the canonical clone's path is never
recorded. The repo row then labels itself with the agent-deck path while
the canonical clone shows up *below* it as a `Checkout` row, which inverts
the conceptual ownership and is visually misleading.

Concretely, an operator with a `~/src/config` clone and an agent-deck
worktree at `~/.agent-deck/multi-repo-worktrees/<id>/config` (both pointing
at the same `~/src/config/.git` common dir) sees the repo row stamped with
the agent-deck path and the canonical checkout nested under it as if it
were the derived one. The visible structure misrepresents which checkout is
the source of truth.

## Decision

For non-bare repos — those whose `common_dir` ends with `/.git` — the
sessions row builder always derives the repo row's display path from the
common dir's parent (`common_dir.strip_suffix("/.git")`), regardless of
what is in `RepoNode.source_paths`. The canonical checkout root is defined
by the location of the `.git` directory itself, so this path is stable
across providers and probe ordering.

For bare repos — those whose `common_dir` does not end with `/.git`, for
example `~/srv/git/foo.git` — there is no canonical checkout location
implied by the common dir, so the builder falls back to
`source_paths.first()` and ultimately to the unmodified common dir.

This decision is scoped to the sessions row builder's display label
(`tui::rows::sessions::repo_display_path`). It does not change `RepoNode`
storage, `source_paths` semantics, or any other consumer of those fields.

## Consequences

- Repo group rows in the sessions view are stable regardless of which
  provider probes a repo first. Adding or removing scan roots cannot
  silently rewrite which path is treated as the repo's "home".
- The canonical clone and any non-canonical worktrees (linked worktrees,
  agent-deck multi-repo checkouts, scratch clones) all appear as
  `Checkout` rows beneath a repo row that points at the canonical clone.
  This matches how operators reason about repo identity.
- Bare repos preserve the previous behavior, which is the only case where
  `source_paths` carries information the common dir does not.
- `RepoNode.source_paths` remains accurate to what was probed but is no
  longer the primary display source for non-bare repos. Other consumers
  that still read `source_paths.first()` are not affected by this ADR and
  may want to revisit their own heuristics separately.

## Alternatives Considered

- **Fix `merge_fragments` to union `RepoNode.source_paths` across
  fragments, then keep preferring `source_paths.first()`.** This makes
  `source_paths` more honest but does not pick a canonical path among
  multiple probed checkouts. The row still needs a tiebreak rule, and the
  common-dir-derived path is the natural answer. Worth doing
  independently, but it does not replace this decision.
- **Prefer the `source_path` that matches the canonical checkout root,
  fall back to `source_paths.first()`, then to the common dir.** Less
  aggressive, but in the bug case `source_paths` does not contain the
  canonical path at all (because of the merge behavior above), so the fix
  silently degrades to the broken behavior. Rejected for that reason.
- **Always show the git common dir verbatim.** Rejected because it leaks
  the `/.git` suffix into the operator-facing UI and adds no information.
- **Track a per-repo "preferred display path" attribute in `RepoNode` and
  let providers fight over it.** Rejected as premature: the canonical
  checkout root is a deterministic function of the common dir for
  non-bare repos, so a separate attribute would only add ceremony.

## Open Questions Answered

- **Does this change what is stored on `RepoNode`?** No. It changes only
  how the sessions row builder picks a display path. `source_paths` keeps
  its meaning as "checkout roots observed by some probe".
- **What about bare repos?** They keep the existing
  `source_paths.first()` fallback, since the common dir does not imply a
  canonical checkout location.
- **Does this affect `node_id_path` (launch-context highlighting)?** No.
  `node_id_path` already uses `repo_display_path_from_common_dir` for
  `NodeId::Repo`, so launch-context matching is consistent with the new
  display rule.
