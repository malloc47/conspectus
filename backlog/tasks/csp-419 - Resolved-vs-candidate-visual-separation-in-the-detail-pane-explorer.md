---
id: CSP-419
title: Resolved-vs-candidate visual separation in the detail-pane explorer
status: Done
assignee: []
created_date: '2026-06-17 19:04'
labels:
  - h-ui
milestone: m-11
dependencies: []
ordinal: 359000
---

## Final Summary

<!-- SECTION:FINAL_SUMMARY:BEGIN -->
ADR 0075 records the edge-state visual language and
the renderer ships it. CSP-417's validated / Other zone
split already separated Resolves from the rest; this story
closes the per-row distinction inside Other. `AltOf(_)` rows
render in `theme.edge_alt_of` (default DarkGray, quiet —
candidates the resolver considered but didn't pick).
`Conflict` rows render with a leading `⚠ ` prefix in
`theme.edge_conflict` (default Yellow) + BOLD; the `⚠`
reuses ADR 0071 / 0072's ambiguity vocabulary and survives
`NO_COLOR`. Unresolved stubs keep their `— ` prefix + DIM
treatment. The legacy `★` resolver-winner marker exits the
renderer entirely (validated zone is the winner zone by
construction). Two new flat `[tui.theme]` color keys
(`edge_alt_of`, `edge_conflict`) ship in `Theme::known_keys`
so operators theme edge states independently of the broader
secondary / warning palette. Candidate-only group fan-outs
do not get a dedicated chip — the per-row treatment + the
Other header's `K ⚠` summary cover the use case. Unit tests
pin the per-edge-state row dispatch (`⚠` prefix on Conflict,
color match on AltOf, no `★` on validated). CSP-420 stays
open as the resolver-side preservation work; this story is
purely renderer.
<!-- SECTION:FINAL_SUMMARY:END -->

Legacy ID: `H-UI-005`
