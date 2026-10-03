---
id: CSP-323
title: Distinguish symmetric relations in the detail explorer
status: To Do
assignee: []
created_date: '2026-06-01 15:17'
labels:
  - t8
milestone: m-13
dependencies:
  - CSP-313
ordinal: 428000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
- Scope: today the explorer buckets edges into Upstream /
  Downstream from the underlying `GraphLink`'s `source → target`
  direction, which reads correctly for directional relations
  (`process_identifies_session`, `runs_in_mux`, …) but is
  misleading for symmetric relations such as `associated_with`
  where the side a link lands on is an artifact of link-builder
  order. Tag each `RelationKind` with a
  `Directionality::{Directed, Symmetric}` and render symmetric
  relations in a third **Related** zone between Upstream and
  Downstream so the layout no longer implies a direction the
  model doesn't carry. Update Node-zone cursor walk order and
  breadcrumb hop carry-state accordingly.
- Tests: reducer tests asserting that symmetric relations land
  in the Related zone (not Upstream or Downstream); cursor walk
  tests covering Node → Upstream → Related → Downstream order;
  renderer snapshot for a node carrying at least one symmetric
  relation; coverage for a node with *only* symmetric edges
  (Upstream and Downstream should suppress, Related should
  render alone).
- Blockers: CSP-313 modeling.
<!-- SECTION:DESCRIPTION:END -->

Legacy ID: `T8-037`
