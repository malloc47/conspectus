---
id: CSP-793
title: 'Name a non-bare repo by its directory in the breadcrumb, not `.git`'
status: Done
assignee:
  - '@claude'
created_date: '2026-10-30 09:41'
updated_date: '2026-10-31 01:01'
labels:
  - tui
  - bug
  - tui-framework
milestone: m-25
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
- [x] #1 A hop for a non-bare repo is tagged by the repo's working directory name (for example `project` for `~/repos/project/.git`); a bare repo keeps its directory name
- [x] #2 Two different repos with one tag show suffixes that differ
- [x] #3 Tests in `explorer_tests.rs` cover a bare repo, a non-bare repo, and two non-bare repos with one directory name
<!-- AC:END -->

## Definition of Done
<!-- DOD:BEGIN -->
- [x] #1 `just check` passes, or `git diff --check` for docs-only changes
- [x] #2 Docs and ADRs are updated when behavior, architecture, or workflow change
<!-- DOD:END -->

## Implementation Plan

<!-- SECTION:PLAN:BEGIN -->
1. `short_node_label`: a repo whose common dir ends in `.git` is named by the directory above it (its working directory); any other (bare) repo keeps its last segment.
2. The suffix: the end of the node id is the end of the path for repos (and checkouts, workspaces, branches), which two with one directory name share. Make it four characters of the `id` row the node's pane shows: the start of the short id for the kinds whose `id` row is one, the end of the key or name for sessions, muxes, and pins (unchanged).
3. Tests in `explorer_tests.rs`: a bare repo, a non-bare repo (with and without a trailing slash), two non-bare repos with one directory name (suffixes differ and match their `id` rows), and a chain where no tag repeats (no suffix).
4. Capture the CSP-790 drills before and after at 160, 100, 60, and 40 columns; docs and review row.
<!-- SECTION:PLAN:END -->

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
Frames (/tmp/tuifw-baseline/frames-793-{before,after}, CSP-790's drills at 160/100/60/40 columns): only title rows differ. Three hops: `◇ project › ◆ .git` became `◇ project·d811 › ◆ project·c955` (checkout and repo share a tag, so CSP-790's rule shows both suffixes; at 40 columns `◆ project·…`). Five hops: `◆ .git·.git` became `◆ repo-a`. Full isolated suite 4804 passed, 0 failed, 1 ignored; fmt, both clippy runs, doc, examples, diff --check pass.
<!-- SECTION:NOTES:END -->

## Final Summary

<!-- SECTION:FINAL_SUMMARY:BEGIN -->
A repo's breadcrumb hop is named by its directory: a non-bare repo by its working directory (`project` for `~/repos/project/.git`), a bare repo by its own (`project.git`). Two hops with one name carry suffixes that differ: four characters of the `id` row the node's pane shows, the start of the short id for repos and the other kinds whose `id` row is a short id (checkouts, workspaces, branches, forks, processes, PRs; the end of their node id was the end of a path or ref, which two in different places share), the end of the key or name for sessions, muxes, and pins (unchanged).

- `repo_dir_name` in `short_node_label` and `hop_suffix` in `src/tui/explorer.rs`.
- Tests in `explorer_tests.rs`: `a_repo_hop_is_named_by_its_directory_whether_bare_or_not`, `two_repos_with_one_directory_name_carry_suffixes_that_differ`, `a_repo_named_once_in_the_chain_shows_no_suffix`.
- Docs: operations guide (the drilled title), changelog, operator-review row.

**Regressions:**
- A checkout and its repo usually share a name now, so a chain through both shows both suffixes under CSP-790's same-tag rule (`◇ project·d811 › ◆ project·c955`, where it read `◇ project › ◆ .git`), and the longer title gives way sooner: at 100 columns the three-hop chain drops its first hop, and at 40 the last hop is cut (`◆ project·…`). Flagged in operator-review with the CSP-790 reversal (compare the glyph with the tag).

**Consistency wins:**
- A non-bare repo's hop reads its project name instead of `.git` (frames `three-*`, `dup-*`).
- Two repos with one name read apart (`◆ .git·.git` twice became distinct names or distinct `·xxxx` suffixes that match each pane's `id` row); checkouts, workspaces, and branches with one name do too.

Checked with `conspectus tui --snapshot` at 160, 100, 60, and 40 columns (`/tmp/tuifw-baseline/frames-793-*`): only the right pane's title row changed. Full isolated suite 4804 passed, 0 failed, 1 ignored; fmt, both clippy runs, doc, examples, and `git diff --check` pass.
<!-- SECTION:FINAL_SUMMARY:END -->
