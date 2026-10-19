---
id: CSP-638
title: Show whether a PR has a local branch; filter and rank by it
status: To Do
assignee: []
created_date: '2026-10-19 19:25'
labels:
  - tui
  - forge
dependencies: []
priority: medium
type: feature
ordinal: 696000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
In the PRs view with `repo` grouping, a PR whose head branch is not a local branch (a merged PR whose branch was deleted, a teammate's PR, a PR in a repo with no matching local remote) lands under a top-level "No local repo" group (`Bucket::NoRepo` in `src/tui/rows/prs.rs`). The label is wrong, since the repo is usually cloned, and a catch-all group hides which PRs matter locally. Operator request: drop that catch-all in favour of a per-row status symbol that distinguishes PRs with a local tracking branch from those without, in the style of the pin marker on mux rows, plus a filter and a sort/rank control so untracked PRs can be included, excluded, or pushed down. Default shows every PR, so all open PRs stay visible.

Open design points for the ADR: how "local tracking branch" is defined (today: the PR links to a Branch node because its head repo is one of the repo's remotes and the head branch exists locally); where PRs without a branch are filed under `repo` grouping; glyph choice and its ASCII fallback; whether the control lives in `RowFilter` (narrowing predicate plus ordering toggle, like `float_attached_muxes_top`) and the Controls overlay.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 An ADR records the definition of a local tracking branch, the filter and ranking controls, and the symbol choice; ADR 0114 is amended where it names the No local repo group
- [ ] #2 Each PR row in the left pane shows one symbol when a local tracking branch exists and a different one when it does not, styled from the theme
- [ ] #3 The PRs view Controls overlay offers a filter over tracked, untracked, or both, defaulting to both, and a toggle that floats PRs with a local branch above those without
- [ ] #4 Both settings persist with the other per-view filter state and are cleared by Clear all filters
- [ ] #5 Under repo grouping a PR with no local branch is filed under its own repo (owner/repo from its remote) instead of a No local repo group
- [ ] #6 conspectus table prs exposes the tracking state, and the filter applies there too if the other filters do
- [ ] #7 Row-tree, filter, and rendering tests cover tracked, untracked, mixed, and sparse cases; tui --snapshot output is checked for the symbols
- [ ] #8 docs/design.md and the TUI controls docs describe the new controls
<!-- AC:END -->

## Definition of Done
<!-- DOD:BEGIN -->
- [ ] #1 `just check` passes, or `git diff --check` for docs-only changes
- [ ] #2 Docs and ADRs are updated when behavior, architecture, or workflow change
<!-- DOD:END -->
