---
id: CSP-269
title: Ship preset theme variants on top of ADR 0032
status: To Do
assignee: []
created_date: '2026-05-25 02:00'
labels:
  - t8
milestone: m-13
dependencies: []
ordinal: 461000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
- Scope: layer named palette presets (`tokyo-night`, `dracula`,
  `solarized-light`, `default-dark`) on top of the flat
  `[tui.theme]` schema. Implementation can stay purely additive
  (config-snippet files shipped in `examples/` that operators
  paste into their config) before any code-level
  `[tui.theme.preset] = "tokyo-night"` selector lands. The latter
  needs a small follow-on ADR clarifying preset precedence vs
  per-key overrides.
- Tests: parse-and-apply tests over each shipped snippet; visual
  diff snapshots for one representative session view per preset
  (only after preset selector wiring lands).
- Blockers: ADR 0032 (landed).
<!-- SECTION:DESCRIPTION:END -->

Legacy ID: `T8-023`
