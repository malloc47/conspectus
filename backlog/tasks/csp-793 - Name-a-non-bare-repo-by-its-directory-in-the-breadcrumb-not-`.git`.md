---
id: CSP-793
title: 'Name a non-bare repo by its directory in the breadcrumb, not `.git`'
status: To Do
assignee: []
created_date: '2026-10-30 09:41'
labels:
  - tui
  - bug
dependencies: []
priority: low
ordinal: 879000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Found while capturing CSP-790's frames. A breadcrumb hop's tag comes from `short_node_label`, which names a repo by the last segment of its `common_dir`. For every non-bare repo that is `.git`, so the right pane's title reads `◆ .git` for any repo the operator drilled through. The suffix that should tell two hops with one tag apart is the last four characters of the node id, which for a non-bare repo is `.git` too, so a chain through two repos reads `◆ .git·.git › … › ◆ .git·.git` and the suffix tells nothing apart. Bare repos (`/srv/git/project.git`) read well today.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 A hop for a non-bare repo is tagged by the repo's working directory name (for example `project` for `~/repos/project/.git`); a bare repo keeps its directory name
- [ ] #2 Two different repos with one tag show suffixes that differ
- [ ] #3 Tests in `explorer_tests.rs` cover a bare repo, a non-bare repo, and two non-bare repos with one directory name
<!-- AC:END -->

## Definition of Done
<!-- DOD:BEGIN -->
- [ ] #1 `just check` passes, or `git diff --check` for docs-only changes
- [ ] #2 Docs and ADRs are updated when behavior, architecture, or workflow change
<!-- DOD:END -->
