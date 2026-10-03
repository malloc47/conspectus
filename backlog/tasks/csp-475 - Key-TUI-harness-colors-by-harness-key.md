---
id: CSP-475
title: Key TUI harness colors by harness key
status: Done
assignee: []
created_date: '2026-07-01 22:56'
labels:
  - h-ext
milestone: m-11
dependencies:
  - CSP-474
ordinal: 157000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
- Landed 2026-07-03. Theme replaces the four flat
  `harness_claude` / `harness_codex` / `harness_opencode`
  / `harness_aider` `Color` fields with
  `harness_colors: BTreeMap<String, Color>` keyed by the
  canonical harness key. `harness_unknown` stays a
  top-level fallback. `Theme::harness_color(label_or_key)`
  walks the adapter registry so callers can pass either a
  harness key (`claude-code`) or a display label (`claude`);
  both resolve to the same map entry with `harness_unknown`
  as the miss fallback.
  Config gets a new `[tui.theme.harness]` nested table
  handler in `merge_tui_theme_harness`: each `<key> =
  "color"` sets `theme.harness_colors[<key>]`; unknown
  per-key entries emit a diagnostic that lists the
  registered set. The pre-H-EXT-003 flat aliases
  (`harness_claude` etc.) still work — `set_color`
  grandfathers them to the corresponding map entry per
  ADR 0031's precedent.
  Viewer callers migrated to the registry-aware lookup:
  `viewer/widget.rs::harness_chip_style` collapses its
  per-key match into `theme.harness_color(harness)`;
  `viewer/render.rs`'s user-turn color and the viewer
  help-sheet's key column route through
  `theme.harness_color("codex")` so an operator override
  at `[tui.theme.harness].codex` also recolors those
  semantic reuses.
  Config test suite gets three new anchor tests:
  `tui_theme_harness_table_overrides_per_key_colors`,
  `tui_theme_harness_table_unknown_key_emits_diagnostic`,
  `tui_theme_flat_harness_alias_still_works`. Existing
  parse tests migrated to the registry-aware
  `harness_color(key)` accessor. `examples/pantry.rs`
  updated to preview the new
  `[tui.theme.harness].<key>` shape.
  All 25 suites (1517 lib tests) pass; fmt / clippy clean.
- Blockers: `CSP-474` (landed).
<!-- SECTION:DESCRIPTION:END -->

Legacy ID: `H-EXT-003`
