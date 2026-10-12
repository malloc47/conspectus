---
id: CSP-592
title: >-
  Link a pull request to a local branch only when its head is one of this repo's
  remotes
status: Done
assignee:
  - '@claude'
created_date: '2026-10-12 23:35'
updated_date: '2026-10-12 23:54'
labels:
  - p4-fu
milestone: m-9
dependencies: []
priority: high
ordinal: 641000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
- Why: branch matching uses the head branch name alone, so a PR from someone else's fork with a branch also named `main` links to the local `main`.
- Scope: require the PR's head owner/repo to match one of the local repo's GitHub remotes before linking to a local branch; otherwise keep the link unresolved with its evidence.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [x] #1 A PR whose head lives in one of the repo's remotes links to the matching local branch
- [x] #2 A PR whose head lives in an unrelated fork stays an unresolved link, even when a local branch has the same name
- [x] #3 Tests cover a same-repo PR, a PR from the operator's fork into upstream, and a stranger's fork
<!-- AC:END -->

## Definition of Done
<!-- DOD:BEGIN -->
- [x] #1 `just check` passes, or `git diff --check` for docs-only changes
- [x] #2 Docs and ADRs are updated when behavior, architecture, or workflow change
<!-- DOD:END -->

## Implementation Plan

<!-- SECTION:PLAN:BEGIN -->
Link to a local branch only when the PR head owner/repo is one of the repo's GitHub remotes; a missing head repo counts as the queried repo.
<!-- SECTION:PLAN:END -->

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
- `gh` omits the head repo only when the fork was deleted; that case counts as the queried repo, so same-repo PRs keep linking.
<!-- SECTION:NOTES:END -->

## Final Summary

<!-- SECTION:FINAL_SUMMARY:BEGIN -->
A PR links to a local branch only when its head lives in one of the repo's GitHub remotes; a stranger's fork with a same-named branch stays an unresolved link with its evidence. Verified with `just check` (same-repo, own-fork-into-upstream, and stranger's-fork tests).
<!-- SECTION:FINAL_SUMMARY:END -->
