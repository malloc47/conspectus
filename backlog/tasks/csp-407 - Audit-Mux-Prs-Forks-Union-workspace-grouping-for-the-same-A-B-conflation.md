---
id: CSP-407
title: Audit Mux/Prs/Forks/Union workspace grouping for the same (A)/(B) conflation
status: Done
assignee: []
created_date: '2026-06-09 14:15'
labels:
  - h-ws
milestone: m-11
dependencies: []
ordinal: 242000
---

## Final Summary

<!-- SECTION:FINAL_SUMMARY:BEGIN -->
The audit found the four views' `Workspace` grouping
variants are unimplemented, not buggy. `src/tui/rows/mux.rs:191`
matched `Session | Workspace | Host` together and called
`emit_flat`; `PrsBuildInputsFromConn`,
`ForksBuildInputsFromConn`, and `UnionBuildInputsFromConn`
carry no `grouping` field at all and never read their
respective grouping enums. The `Workspace` cycler entries
therefore advertised a label that selected the default flat
layout. There was no (A)/(B) conflation to fix because there
was no workspace nesting to fix. Decision (ADR 0061): drop the
`Workspace` variant from `MuxGrouping`, `UnionGrouping`,
`PrsGrouping`, and `ForksGrouping`; the Workspaces view
(`CSP-406`) is the canonical workspace-first surface, and the
(A)/(B) distinction does not translate cleanly to Prs/Forks
(no cwd → no analog of "workspace-rooted"). `Grouping::as_str`,
`Grouping::values_for`, and the dead match arm in `mux.rs` are
updated; the `workspace_chip: None` comments in the four row
builders now record that the chip has no analog in views
without workspace grouping. Configs that set
`grouping = "workspace"` on these views now produce a
`ConfigDiagnostic` listing the valid menu values rather than
silently mapping to flat. No new row-tree tests are needed;
the existing `parse_and_as_str_round_trip_per_view` test
iterates `values_for(view)` and continues to pass over the
shrunken menus.
<!-- SECTION:FINAL_SUMMARY:END -->

Legacy ID: `H-WS-003`
