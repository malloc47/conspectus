---
id: CSP-269
title: Ship preset theme variants on top of ADR 0032
status: To Do
assignee: []
created_date: '2026-05-25 02:00'
updated_date: '2026-10-31 08:15'
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

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
TUI framework update (2026-10-31, CSP-766): the theme is the framework's (`tuikit::theme`, ADR 0153): generic tokens, the host's named roles (`ROLES` in `src/tui/theme.rs`, harness colors as `harness.<key>`), and old keys kept as `ALIASES`; `[tui.theme]` and `[tui.theme.harness]` are walked once, with problems reported as config warnings. A preset is a configuration table applied before the user's own (`crates/tuikit/docs/controls/theme.md`, "Seams left open"), which settles the precedence the follow-on ADR was to decide: the preset, then the user's keys. Light and dark adaptation would choose between presets and is not planned until they exist. Honoring `NO_COLOR` and the terminal's color depth is CSP-803.
<!-- SECTION:NOTES:END -->
