---
id: CSP-084
title: Share the relation-kind string codec
status: Done
assignee: []
created_date: '2026-05-17 20:45'
labels:
  - h-ref
milestone: m-11
dependencies: []
ordinal: 84000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
- Landed 2026-07-05 (`3dd6a2c`). Added
  `RelationKind::from_snake_case` inverse method next to
  the existing `snake_case()`. `parse_relation_kind` /
  `relation_label` deleted from cli.rs. 22-variant
  round-trip test + unknown-label test in `model_tests.rs`.
- Blockers: none.
<!-- SECTION:DESCRIPTION:END -->

Legacy ID: `H-REF-002`
