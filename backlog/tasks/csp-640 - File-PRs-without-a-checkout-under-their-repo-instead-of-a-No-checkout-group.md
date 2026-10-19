---
id: CSP-640
title: File PRs without a checkout under their repo instead of a No checkout group
status: Done
assignee:
  - '@claude'
created_date: '2026-10-19 21:36'
updated_date: '2026-10-19 21:45'
labels:
  - tui
dependencies: []
priority: medium
type: enhancement
ordinal: 698000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
The PRs view's default checkout grouping puts every PR whose branch is not checked out anywhere under one top-level "No checkout" group (ADR 0114). With enterprise hosts queried (ADR 0128) that group holds most PRs, the same long catch-all list the repo grouping had before ADR 0129. The tracking marker already separates PRs with a local branch from those without, so the group no longer needs to carry that meaning.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [x] #1 Under checkout grouping, a PR with no checkout sits under a group for its repo, labelled so it is distinct from that repo's checkout groups; no No checkout group is emitted
- [x] #2 Checkout groups still come first and a PR with a checkout is filed as before
- [x] #3 A sparse graph with no local repo for the PR falls back to the owner/repo group
- [x] #4 An ADR records the change, amending 0114 and 0129; design.md and tests are updated and the PR frame snapshots reviewed
<!-- AC:END -->

## Definition of Done
<!-- DOD:BEGIN -->
- [x] #1 `just check` passes, or `git diff --check` for docs-only changes
- [x] #2 Docs and ADRs are updated when behavior, architecture, or workflow change
<!-- DOD:END -->

## Final Summary

<!-- SECTION:FINAL_SUMMARY:BEGIN -->
Under checkout grouping, a PR checked out nowhere is filed under a group for its repo, labelled `<path>  no checkout`, in place of the old top-level "No checkout" group; checkout groups still come first and a sparse graph falls back to the `owner/repo` group. ADR 0130 amends 0114 and 0129, and `design.md` and the changelog are updated. Verified with fmt, clippy `-D warnings`, the full nextest suite (2503 passed; new row-builder tests, PR frame snapshot headers reviewed) and `tui --snapshot` against this machine's 90-PR world.
<!-- SECTION:FINAL_SUMMARY:END -->
