# ADR 0129: PRs Show Whether They Have A Local Branch, With A Filter And A Ranking

## Status

Accepted. Amends ADR 0114 (the `repo` grouping's "No local repo" group)
and extends ADR 0031 (the per-view filter and the Controls overlay).
Amended by ADR 0130: the config keys and the `checkout` grouping, which
this ADR left alone, are covered there. Amended by ADR 0131: the marker
follows the PR number instead of the kind glyph.

## Context

A PR links to a local branch only when its head repo is one of the
repo's remotes and the head branch exists locally (ADR 0115, item 4).
Most PRs fail that test: a merged PR whose branch was deleted, a
teammate's PR, a fork's PR. Once forge discovery covers enterprise
hosts (ADR 0128) most of the PRs view is of that kind.

ADR 0114 filed those PRs under a top-level "No local repo" group in the
`repo` grouping. The label is wrong, since every PR comes from a repo
Conspectus discovered on disk; the group says nothing about which PRs
matter to local work; and it takes the place a repo header should hold.

## Decision

1. **"Tracked" means the PR resolves to a local Branch node.** It is the
   existing `branch_has_forge_pr` link whose target is a branch in the
   graph. Nothing about discovery or resolution changes.
2. **A status symbol marks every PR row in the left pane**, directly
   after the PR's kind glyph, in the way the pin marker distinguishes a
   bound pin from an unbound one:
   - tracked: the Branch kind glyph (`⎇` by default, through the same
     icon overrides and the `node_branch` color);
   - untracked: a dim hollow `◌`, the shape the unbound-pin marker uses
     but in the dim placeholder style, since most PRs are untracked and
     a bright marker on each would be noise.
3. **A filter includes or excludes by tracking.** `RowFilter` gains
   `pr_branch: Option<PrBranchFilter>` with `Tracked` and `Untracked`.
   `None`, the default, shows every PR, so every open PR stays visible.
   It counts toward `RowFilter::is_empty` (so "Clear all filters"
   resets it and the empty-state message names it) but not toward
   `has_narrowing_predicates`, which gates the harness, age, and mux
   predicates that decide which attached sessions a PR shows. The
   status bar shows it as a `branch:tracked` or `branch:untracked` chip.
4. **A ranking floats tracked PRs to the top.** `RowFilter` gains
   `float_tracked_prs_top: bool`, an ordering toggle like
   `float_attached_muxes_top`. Within the tracked and untracked halves
   the existing order holds, and it applies inside each group.
5. **Controls and CLI.** The PRs view's Controls overlay gets a
   "local branch" row that cycles any, tracked, untracked on Enter, and
   a "Float PRs with a local branch to top" checkbox. Both persist with
   the other per-view filter state. `conspectus tui` and `conspectus
   table prs` accept `--pr-branch tracked|untracked`. The config file's
   `[tui.views.*.filters]` table is not extended.
6. **`repo` grouping files every PR under its local repo.** The
   unresolved branch link that an untracked PR keeps as evidence now
   records the local repo it was queried for (`local_repo`, the repo's
   common dir). The builder uses it when no branch resolved, so the
   PR sits under the same repo header as its tracked siblings. A PR
   with neither a branch nor that evidence (a sparse graph) is filed
   under its remote's `owner/repo`. The "No local repo" group goes away.
7. **`table prs` gains a `tracked` column** (`yes` or `no`); the
   `branch` column keeps showing the branch name.
8. **`checkout` grouping is unchanged.** Its "No checkout" group holds
   PRs whose branch isn't checked out anywhere, tracked or not; the
   symbol tells them apart.

## Consequences

- The default view loses nothing: filtering and ranking are opt-in.
- A PR's row carries one more cell, and the `unresolved` link evidence
  one more metadata key, so the graph JSON for untracked PRs changes.
- `has_narrowing_predicates` keeps its meaning; new PR-only predicates
  must not be added to it.
- The tracking filter has no config-file key. Operators who want it by
  default set it once in the overlay, and the TUI remembers it.

## Alternatives Considered

- **Keep a catch-all group and rename it "No local branch".** Honest,
  but it still pulls the PRs that matter least to the front of a list
  and splits one repo's PRs across two groups.
- **Hide untracked PRs by default.** Cleaner for a machine full of
  teammates' PRs, but it hides open PRs the operator may want to review.
- **A new sort mode instead of a toggle.** The sort control is global
  across views and has two values; a per-view float toggle matches the
  Sessions and Mux views.
- **Fold the filter into `has_narrowing_predicates`.** Makes the PRs
  builder drop every PR without an attached session as soon as the
  filter is set.
- **Trailing marker, as on pinned muxes.** A leading marker stays in a
  fixed column whatever the length of the row, which matters because the
  PR text is long and variable.
