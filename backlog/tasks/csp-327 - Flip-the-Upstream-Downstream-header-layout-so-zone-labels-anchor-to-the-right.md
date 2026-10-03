---
id: CSP-327
title: >-
  Flip the Upstream / Downstream header layout so zone labels anchor to the
  right
status: Done
assignee: []
created_date: '2026-06-01 15:17'
labels:
  - t8
milestone: m-13
dependencies:
  - CSP-315
ordinal: 432000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
- Scope: today the explorer's zone headers render the bold
  `Upstream` / `Downstream` label on the left and the aggregate
  summary on the right (`Downstream  2 groups · 3 links · 1 ⚠`),
  so the highlighted label gets pushed toward the center of the
  pane and is hard to scan vertically when the right pane is
  narrow. Flip the order so the aggregate counts render on the
  left and the bold label anchors flush right
  (`2 groups · 3 links · 1 ⚠  Downstream`). Apply the same flip
  to the third **Related** zone introduced by `CSP-323` if it
  lands first.
- Tests: renderer snapshot for a wide pane (aggregate left,
  label flush right); snapshot for a narrow pane (label still
  visible, aggregate elided rather than the label); coverage
  for Upstream, Downstream, and Related zones; regression that
  empty zones still suppress entirely.
- Blockers: `CSP-315` renderer.
<!-- SECTION:DESCRIPTION:END -->

Legacy ID: `T8-041`
