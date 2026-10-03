---
id: CSP-314
title: Replace inline expansion with relationship-group navigation
status: Done
assignee: []
created_date: '2026-06-01 00:28'
labels:
  - t8
milestone: m-13
dependencies:
  - CSP-313
ordinal: 418000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
- Scope: change `e` to expand/collapse relationship groups only.
  Add right-pane cursor state for relationship rows. `Enter` drills
  into the selected neighbor node, `Backspace` returns through a
  breadcrumb stack, and selection survives refreshes by node id plus
  selected relationship id where possible. Remove or retire the
  existing "expanded linked details" state from `CSP-300`.
- Tests: reducer/keymap tests for group expand/collapse, drilldown,
  back navigation, breadcrumb reset on missing nodes, and refresh
  stability.
- Blockers: `CSP-313`.
<!-- SECTION:DESCRIPTION:END -->

Legacy ID: `T8-028`
