# ADR 0134: A PR State Filter

## Status

Accepted. Extends ADR 0129 (the PRs view's filters) and ADR 0031 (the
per-view filter and the Controls overlay).

## Context

The PRs view's `state` grouping sorts PRs into open, draft, merged, and
closed groups, but the common wish is the opposite: see only the open
PRs while keeping the `checkout` or `repo` grouping. With enterprise
hosts queried (ADR 0128), merged and closed PRs make up most of the
list, so showing only the live ones is the usual first step.

## Decision

1. **A PR state set filter.** `RowFilter` gains
   `pr_state: Option<PrStateFilter>`, a set of `open`, `draft`,
   `merged`, and `closed`, like `mux_state`. `None`, the default, shows
   every PR. A PR passes when it is in any listed state.
2. **`open` includes drafts, as on the forge.** `draft` matches only
   open drafts, so `open` alone is every open PR, `draft` alone is the
   drafts, and a merged or closed draft counts as merged or closed. A
   forge state outside these four (unknown to the adapter) is shown
   only when no state filter is set. State text is compared without
   regard to case.
3. **Like the local-branch filter, it is not a session predicate.** It
   counts toward `RowFilter::is_empty` (so "Clear all filters" resets
   it) but not toward `has_narrowing_predicates`, which decides which
   attached sessions a PR keeps. It applies under every grouping and
   drops groups it empties.
4. **Surfaces.** The PRs view's Controls overlay gets a "PR state"
   multi-select above "local branch"; the status bar shows
   `state:open,merged`; the setting persists with the other per-view
   filter state. `conspectus tui` and `conspectus table prs` accept
   `--pr-state open,merged` (comma-separated or repeated), and
   `[tui.views.<view>.filters]` accepts `pr_state = ["open", "draft"]`
   with entries OR-ing together and a diagnostic for an unknown value.

## Consequences

- "Only open PRs" is one multi-select, or one config line, without
  giving up the grouping.
- An operator who wants open PRs by default can set
  `pr_state = ["open"]` once in the config.
- Combined with the `state` grouping, the filter leaves only the groups
  for the chosen states.

## Alternatives Considered

- **A single-value state toggle.** Simpler to cycle, but "open or
  draft" and "merged or closed" are common asks.
- **Make `open` exclude drafts.** Matches the grouping's buckets, but
  surprises: `is:open` on the forge includes drafts.
- **Hide merged and closed PRs by default.** Hides PRs the operator may
  want to find; ADR 0129 kept the default to show everything.
