---
id: CSP-412
title: Surface node-kind identity in non-TUI outputs (cross-surface consistency)
status: To Do
assignee: []
created_date: '2026-06-09 14:32'
labels:
  - h-vis
milestone: m-17
dependencies:
  - CSP-409
  - CSP-411
ordinal: 524000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
- Scope: extend `NodeId` / `GraphNode` with a `node_kind()` method
  returning the stable string tag (`"repo"`, `"agent_session"`,
  etc.) so non-TUI consumers — `conspectus graph --format json`,
  `conspectus node show`, the HTML explorer, DOT output — receive
  the same node-kind identity. The TUI glyph is a *rendering*
  choice; the stable identifier is a *model* concern. Add the
  machine-readable `node_kind` field alongside the display glyph so
  consumers that can render symbols (HTML with Nerd Font CSS, a TUI
  with a compatible terminal) use the glyph, while JSON consumers
  use the string tag. The DOT renderer already labels nodes by kind;
  this story gives it the node-kind color as a fill or font color.
  The HTML explorer applies the node-kind glyph via a CSS class or
  data attribute.
- Tests: JSON snapshot updates confirming the new field does not
  break existing consumers; DOT snapshot updates confirming node-
  kind colors appear; HTML payload snapshot updates confirming
  glyph/color propagation.
- Blockers: `CSP-409`, `CSP-411`.
<!-- SECTION:DESCRIPTION:END -->

Legacy ID: `H-VIS-005`
