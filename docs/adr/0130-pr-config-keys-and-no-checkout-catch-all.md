# ADR 0130: PR Tracking Config Keys, And No "No checkout" Catch-All

## Status

Accepted. Amends ADR 0129 (items 5 and 8) and ADR 0114 (the `checkout`
grouping's "No checkout" group).

## Context

ADR 0129 left two things out on purpose: a config-file key for the
local-branch filter, and a change to the `checkout` grouping. Both
turned out to matter.

- An operator who always wants untracked PRs hidden, or tracked PRs
  first, had to set it in the Controls overlay on every machine, while
  every other view filter has a `[tui.views.<name>.filters]` key.
- The `checkout` grouping, the PRs view default, files every PR whose
  branch isn't checked out under one top-level "No checkout" group. With
  enterprise hosts queried (ADR 0128) that group holds most of the PRs,
  the same long catch-all list ADR 0129 removed from the `repo`
  grouping. The tracking marker now says which of those PRs have a local
  branch, so the group no longer has to.

## Decision

1. **Config keys.** `[tui.views.<view>.filters]` accepts
   `pr_branch = ["tracked" | "untracked", ...]` and
   `float_tracked_prs_top = true | false`, as an inline table or an array
   of tables like the other filter keys. Entries OR together per
   dimension: `tracked` with `untracked` is no restriction, and any entry
   that sets the float wins. An unknown `pr_branch` value emits a config
   diagnostic naming the key and the valid values; valid entries still
   apply. The keys are read for any view but only the PRs view acts on
   them, as with `mux_state`.
2. **No "No checkout" group.** Under `checkout` grouping, a PR with no
   checkout is filed under a group for its repo, labelled
   `<path>  no checkout` so it reads as distinct from that repo's
   checkout groups. The repo is the PR's branch's repo when it has a
   local branch, else the local repo recorded on its unresolved branch
   link (ADR 0129, item 6), else the `owner/repo` group. Checkout groups
   still come first.

## Consequences

- A configured default carries to every machine the config does, and the
  Controls overlay still overrides it for the session.
- One more kind of group can appear in the default PRs view. A repo
  with both a checked-out PR and an unchecked one shows two headers, a
  checkout header and a `no checkout` repo header, told apart by glyph
  and label.
- Group counts and expansion state are keyed by group, so a PR moving
  from "No checkout" to its repo group collapses or expands with that
  group rather than the old one.

## Alternatives Considered

- **Keep "No checkout" and rely on the marker.** Least change, but it
  leaves the single long list this ADR exists to remove.
- **Nest the unchecked PRs under their repo's checkout group.** Puts PRs
  under a checkout they are not checked out in, and a repo with no
  checkout at all would still need a home.
- **A new `repo` fallback only for untracked PRs.** Splits tracked and
  untracked PRs that are both unchecked-out across two groups for no
  gain.
- **Config keys for the other float toggles too.** Out of scope; those
  views have no request for it.
