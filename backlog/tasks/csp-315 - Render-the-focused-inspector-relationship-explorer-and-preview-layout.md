---
id: CSP-315
title: 'Render the focused inspector, relationship explorer, and preview layout'
status: Done
assignee: []
created_date: '2026-06-01 00:28'
labels:
  - t8
milestone: m-13
dependencies:
  - CSP-314
ordinal: 419000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
- Scope: update the right-panel renderer so core node facts, grouped
  relationships, and selected-edge/neighbor preview have distinct
  visual treatment. Avoid nested section dividers in previews.
  Truncate long mux names, process observation keys, commands, and
  transcript paths in rows while preserving access to the full value
  through the focused preview or a follow-up full-value overlay.
- Tests: Ratatui buffer snapshots for the cluttered mux/process
  case, narrow terminals, long labels, expanded groups, and
  breadcrumb drilldown.
- Blockers: `CSP-314`.
<!-- SECTION:DESCRIPTION:END -->

Legacy ID: `T8-029`
