---
id: CSP-654
title: Sort mux workspace and repo headers together by name
status: Done
assignee:
  - '@claude'
created_date: '2026-10-21 20:53'
updated_date: '2026-10-21 21:00'
labels:
  - tui
  - workspace
dependencies: []
references:
  - src/tui/rows/mux.rs
  - docs/adr/0139-mux-workspace-or-repo-grouping.md
priority: medium
type: enhancement
ordinal: 712000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
The mux view's `workspace` grouping (ADR 0139, CSP-653) lists every workspace header before every repo header, then sorts each kind separately: workspaces by label, repos by full display path. The operator wants one alphabetical list in which neither workspaces nor repos are favoured, so a `conspectus` repo header sits between workspaces named `alpha` and `documentation` rather than after all of them.

The name the operator reads on each header is its bold label: the workspace name for a workspace header, the last path component for a repo header (`compact_path_label` in `src/tui/ui/text.rs`). The Pins group (first) and Ungrouped (last) are synthetic groups, not workspaces or repos, and keep their places.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [x] #1 Under the mux `workspace` grouping, workspace and repo headers form one list sorted case-insensitively by the bold name each header shows, with the full path as a tie-break
- [x] #2 The Pins group stays first and Ungrouped stays last
- [x] #3 The `repo` grouping order is unchanged
- [x] #4 Tests cover interleaved workspace and repo headers, a name tie, and case
- [x] #5 ADR 0139 and docs/design.md describe the new order
<!-- AC:END -->

## Definition of Done
<!-- DOD:BEGIN -->
- [x] #1 `just check` passes, or `git diff --check` for docs-only changes
- [x] #2 Docs and ADRs are updated when behavior, architecture, or workflow change
<!-- DOD:END -->

## Implementation Plan

<!-- SECTION:PLAN:BEGIN -->
1. rows/mod.rs: split the workspace name out of `workspace_header_label` as `workspace_display_name`, so the sort key and the bold label come from one place.
2. mux.rs: extract repo bucketing (`bucket_by_repo`) and the Ungrouped emitter from `emit_repo_grouped`, which keeps its path order.
3. `emit_workspace_grouped`: put workspace and repo buckets in one list keyed by (lowercased name, name, path) where name is the workspace name or the repo path's last component; emit in that order, then Ungrouped. Pins handling is unchanged.
4. Amend ADR 0139 decision 2 and the docs/design.md sentence; update the existing ordering test and add interleave / tie / case tests.
<!-- SECTION:PLAN:END -->

## Final Summary

<!-- SECTION:FINAL_SUMMARY:BEGIN -->
Under the mux `workspace` grouping, workspace and repo headers now form one list sorted case-insensitively by their bold name (workspace name or repo path's last component), full path breaking ties; Pins stays first, Ungrouped last, and `repo` grouping is untouched (its existing tests pass unchanged). ADR 0139 and docs/design.md are amended. Tests: the two existing workspace-grouping tests were updated to the interleaved order and a new one covers case folding and a name tie. On the captured live fixture the headers read conspectus, data-platform, dek, documentation, sysadmin. `just check` passes (2575 tests).
<!-- SECTION:FINAL_SUMMARY:END -->
