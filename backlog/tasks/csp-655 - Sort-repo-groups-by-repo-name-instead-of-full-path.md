---
id: CSP-655
title: Sort repo groups by repo name instead of full path
status: To Do
assignee: []
created_date: '2026-10-21 21:07'
updated_date: '2026-10-31 08:16'
labels:
  - tui
dependencies: []
references:
  - src/tui/rows/mux.rs
  - src/tui/rows/sessions.rs
  - src/tui/rows/mod.rs
priority: medium
type: enhancement
ordinal: 713000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Repo group headers show the repo's name in bold (the last path component, `compact_path_label` in `src/tui/ui/text.rs`) with the full path dimmed after it, but they are ordered by the full path. The operator reads the list by name, so path order looks unsorted: under the mux `repo` grouping `~/src/dek` lists before `~/src/drw/up/documentation/data-platform` (`dek` before `data-platform`) and `~/src/oss/conspectus` lists after both. The operator wants repo groups sorted by repo name only.

Current orderings:
- Mux `repo` grouping and the repo buckets inside the mux `workspace` grouping's fallback: `RepoBucketKey` sorts by display path, then common dir (`src/tui/rows/mux.rs`). Under `workspace` grouping the combined header list already sorts by name (CSP-654, ADR 0139); this task makes `repo` grouping match.
- Sessions view repo groups (`graph`, `repo`, `checkout` groupings): `GroupKey` / `RepoBucket` sort by git common dir, then display path (`src/tui/rows/sessions.rs`). Workspace buckets stay ahead of repo buckets there (ADR 0064) unless the operator asks otherwise.

Use the same key CSP-654 uses for headers (case-insensitive name, then exact name, then full path) so equally named repos in different places keep a stable order.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 Under the mux `repo` grouping, repo headers sort case-insensitively by repo name, with the full path breaking ties
- [ ] #2 Sessions-view repo groups sort the same way within their level, and their checkout rows keep a stable order
- [ ] #3 Pins stays first and Ungrouped last in the mux view; sessions-view workspace buckets keep their place ahead of repos
- [ ] #4 Tests cover name order differing from path order, a name tie, and case
- [ ] #5 docs/design.md, and ADR 0139 or a new ADR as appropriate, describe the order
<!-- AC:END -->

## Definition of Done
<!-- DOD:BEGIN -->
- [ ] #1 `just check` passes, or `git diff --check` for docs-only changes
- [ ] #2 Docs and ADRs are updated when behavior, architecture, or workflow change
<!-- DOD:END -->

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
TUI framework update (2026-10-31, CSP-766): `src/tui/ui/text.rs` and `compact_path_label` are gone. A group header carries its bold name and its dim detail as two cells, `GroupRow.label` and `GroupRow.detail`, filled by the row builders; `GroupRow::at_path` names a header for a path by its last part (`src/tui/rows/mod.rs`, CSP-752, ADR 0184), and the left pane lays the cells out through the views' column schemas (`tuikit::columns`, ADR 0183). The sort keys this story changes (`RepoBucketKey` in `src/tui/rows/mux.rs`, `RepoBucket` in `src/tui/rows/sessions.rs`) are where they were; sorting by the header's `label` sorts by the name the operator reads. References updated: `src/tui/ui/text.rs` replaced by `src/tui/rows/mod.rs`.
<!-- SECTION:NOTES:END -->
