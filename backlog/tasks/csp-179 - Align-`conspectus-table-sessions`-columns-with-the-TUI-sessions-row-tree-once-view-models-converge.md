---
id: CSP-179
title: >-
  Align `conspectus table sessions` columns with the TUI sessions row tree once
  view-models converge
status: To Do
assignee: []
created_date: '2026-05-19 19:33'
updated_date: '2026-10-31 08:15'
labels:
  - t8
milestone: m-13
dependencies:
  - CSP-163
  - CSP-164
ordinal: 445000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
- Scope: today `output::table` builds its own per-projection
  extractors; the TUI sessions row tree introduces a stable
  pure view-model. Wire the table renderer to consume the same
  view-model (or a shared subset) so a single change to the
  sessions sort/grouping rules updates both surfaces. Decide
  whether the TUI's `~`-shortening and harness label
  collapsing should also apply to the CLI table by default,
  behind a `--paths short|full` knob.
- Tests: snapshot parity tests showing TUI row tree and
  `table sessions` produce consistent labels for the same
  snapshot.
- Blockers: `CSP-163` (all five view-models present) and
  `CSP-164` (detail view-models) so the shared API surface is
  settled.
<!-- SECTION:DESCRIPTION:END -->

Legacy ID: `T8-002`

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
TUI framework update (2026-10-31, CSP-766): re-scoped. The TUI's rows now come from column schemas (`tuikit::columns::ColumnSchema`, ADR 0183 to 0185) built by the row builders in `src/tui/rows/`, and the CLI tables from `ColumnSpec` in `src/output/render.rs`. The meeting point is the column schema rather than a view model: the framework's "plain-text tables" seam (`crates/tuikit/docs/controls/columns.md`; roadmap Tier 2) would let `table sessions` lay out from the same schema. What is left: decide whether the CLI renderer adopts the schema or shares only the cell builders and labels. The CLI and the TUI already share the age ladder (`tuikit::age` through `format_relative_age`) and the snippet cut (`tuikit::text::fit_snippet`, CSP-774).
<!-- SECTION:NOTES:END -->
