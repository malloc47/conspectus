# ADR 0114: A PRs View Grouped By Checkout

## Status

Accepted. Amends ADR 0031 §4 (the PRs view's groupings and default)
and reverses `CSP-501`'s hide for the PRs view only.

## Context

`CSP-501` hid the Union, PRs, and Forks views from the TUI because
they confused operators, keeping their row builders and `conspectus
table` projections so the hide could be undone "when those views
mature". The PRs view had two gaps: its builder ignored the grouping
setting, so `repo` and `state` produced the same flat list, and it
rendered no ages. With pull requests now previewing their title,
checks, reviews, and body (ADR 0112, `CSP-585.06`), a list of PRs is
the natural way to reach them, and the question an operator brings to
it is usually "which PRs belong to the work checked out here".

## Decision

1. **The PRs view returns.** It joins Sessions and Mux in the view
   switcher, the controls overlay, and the `[` / `]` cycle, on the `3`
   key. Union and Forks stay hidden.
2. **Grouped by checkout by default.** `PrsGrouping` becomes
   `{ Checkout, Repo, State }`, with `Checkout` first and default:
   each PR sits under the checkout that has its head branch checked
   out, and PRs whose branch isn't checked out anywhere sit under "No
   checkout". `Repo` groups by the local repo the head branch belongs
   to ("No local repo" otherwise), and `State` by open, draft, merged,
   and closed. ADR 0031's planned `Workspace` grouping stays unbuilt.
3. **Groupings are real.** The builder emits a group header per
   bucket, PRs beneath it, and the sessions working in a PR's checkout
   beneath the PR, and it honors the view's filter and renders ages.

## Consequences

- PRs are reachable in one keystroke, and their previews come with
  them.
- A configured or remembered `prs` grouping of `repo` or `state` keeps
  working; the new default only applies when none is set.
- A PR whose branch is checked out in two worktrees appears under the
  first, by root path, so each PR has one row.

## Alternatives Considered

- **Keep the view hidden; reach PRs through Related rows.** Works
  today, but only from a branch or checkout that already links to the
  PR.
- **Default to `repo`.** ADR 0031's original default. Groups more PRs
  together, but separates a PR from the checkout where its work
  happens.
- **Put a PR under every checkout of its branch.** Complete, but
  duplicated rows complicate selection and search for a rare case.
