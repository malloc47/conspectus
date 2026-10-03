---
id: CSP-410
title: Apply node-kind glyphs and colors to the TUI row tree
status: Done
assignee: []
created_date: '2026-06-09 14:32'
labels:
  - h-vis
milestone: m-17
dependencies: []
ordinal: 522000
---

## Final Summary

<!-- SECTION:FINAL_SUMMARY:BEGIN -->
`render_left_row` (`src/tui/ui.rs`) prepends the ADR
0073 node-kind glyph after the disclosure column for every
`RowKind` that carries a graph node identity: Workspace (`▦`)
on the workspace row, Repo (`◆`) for repo group rows and
workspace member-repo rows, AgentSession (`●`) before the
harness pill, MuxSession (`▣`) before the native-id label and
the existing `◉`/`◯` attachability chip, Fork (`⑂`), ForgePr
(`⇄` with `pr_*` state color), and the mux-candidate child row
(also `▣`). Synthetic group buckets and `Pin` rows skip the
prefix — pins keep `📌` as their sentinel identity. The
group-row body-width pre-pass (`group_row_body_width`) folds
the glyph through the same dispatch so summary-chip alignment
stays put across visible groups. Showcase fixture verified by
rendering each view via `conspectus tui --snapshot-fixture
showcase.json --snapshot --snapshot-pane left`; sessions /
mux / prs / forks all show the slate without disclosure-column
drift. Unit tests cover the per-row helpers
(`node_kind_glyph_span`, `forge_pr_glyph_span`, the
`row_kind_glyph_span` dispatch) including the PR
state→color mapping and the Pin / synthetic-group skip cases.
Detail-pane glyph application lands under CSP-411.
<!-- SECTION:FINAL_SUMMARY:END -->

Legacy ID: `H-VIS-003`
