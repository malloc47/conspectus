---
id: CSP-417
title: >-
  Roll back the detail-pane upstream/downstream split; render a single
  related-entities list with descriptive edge labels
status: Done
assignee: []
created_date: '2026-06-17 15:15'
labels:
  - h-ui
milestone: m-11
dependencies: []
ordinal: 357000
---

## Final Summary

<!-- SECTION:FINAL_SUMMARY:BEGIN -->
ADR 0074 records the design and pass 2 lands the
implementation. `NodeView` exposes one `relationships`
surface; `Direction` lives on `RelationshipGroup`; the
verb catalog (`directional_verb`) maps every
`(RelationKind, Direction)` to a surface verb. The renderer
drops the prior `Upstream` / `Downstream` chip dividers and
the per-`(relation, neighbor_kind)` sub-headers; every row
reads as `<verb 22w> <kind-glyph> <neighbor_label>` and
selects as one cursor stop. Validated rows
(`EdgeStateLabel::Resolves`) sit in a flat zone under one
`Related` divider (`N validated · M other`); alternates,
conflicts, and unresolved-evidence stubs collapse under a
`▶ Other (N)` chevron with the per-detail expansion state
on `ExplorerState.other_expanded`. `ExplorerRowKey` /
`ExplorerRow` collapse to `ValidatedLink` + `OtherHeader` +
`OtherLink` + `OtherUnresolved`, dropping `Direction` from
the cursor identity. `ExplorerActivate` toggles the Other
zone on the header and drills on validated/Other links.
Sort is `(NodeKind ordinal, verb, neighbor_label)`. The
`★` resolver-winner marker is dropped from validated rows
(every row there is by definition a winner) and kept in the
Other zone as a hint at would-be picks. Tests: verb-catalog
exhaustiveness, validated/Other zone split, kind→verb→label
sort, reducer drill + breadcrumb still resolve on the new
row keys, and the renderer surfaces verb + glyph + label
inline. Open questions: arrow-suffix for the one symmetric
verb (`associated with`) stays open until a real operator
confusion materializes.
<!-- SECTION:FINAL_SUMMARY:END -->

Legacy ID: `H-UI-003`
