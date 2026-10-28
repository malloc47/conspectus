---
id: CSP-725
title: Extract `chip` and `age`
status: To Do
assignee: []
created_date: '2026-10-27 20:26'
labels:
  - tui-framework
  - tuifw-foundation
milestone: m-25
dependencies:
  - CSP-721
  - CSP-723
  - CSP-722
documentation:
  - docs/plans/tui-framework/inventory/01-design-tokens.md
  - docs/plans/tui-framework/inventory/02-left-pane-tree.md
priority: medium
type: enhancement
ordinal: 811000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Six chip forms share no code: the harness badge, the command badge, the plain `no agent` span, the dim `N hits` and `was in` chips, the toggle and position chips in the viewers, and the header status chips. The search overlay hard-codes a chip width of 10 and ignores `theme.badge_width`. Relative-time formatting has 4 TUI formatters plus 1 in the CLI, with thresholds hard-coded in `rows/mod.rs` (inventory slices 01 and 02).

Scope: Create `tuikit::chip` (a fixed-width `Chip`, a toggle chip, a segmented title, a position chip, and a status chip) and `tuikit::age` (format, bucket, and tint with injectable thresholds and theme). Conspectus builds `harness_badge` and `command_badge` on `Chip`. Record whether the TUI and CLI share one unit ladder (`s/m/h/d`, with or without `w`).

TUI changes:
- Consistency wins: one chip width rule that honors `theme.badge_width`; one age format wherever age appears
- Regressions or behavior changes: age text in rows that formatted differently (search hits, panes) adopts the shared ladder; the search chip width follows the theme instead of a fixed 10

Tests and checks: Snapshots of the left pane (all views), the search overlay, and the viewer header. Tests for each age bucket boundary.

Blockers: The theme, text, and glyph extractions.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 `tuikit::chip` provides the five chip forms and Conspectus's badges are built on it
- [ ] #2 `tuikit::age` formats, buckets, and tints with injectable thresholds, and the four TUI formatters are removed
- [ ] #3 The search overlay uses `theme.badge_width`
- [ ] #4 The decision on sharing the age ladder with the CLI is recorded
- [ ] #5 Every visible change is listed in the final summary under **Regressions** and **Consistency wins**, and was checked with `conspectus tui --snapshot` at a normal and a narrow width
- [ ] #6 The framework crate names no Conspectus type, string, theme field, or domain word, and the independence guard passes
- [ ] #7 The control has a reference page in the framework docs: purpose, usage, key table, theme tokens used, and the house rules it follows
<!-- AC:END -->

## Definition of Done
<!-- DOD:BEGIN -->
- [ ] #1 `just check` passes, or `git diff --check` for docs-only changes
- [ ] #2 Docs and ADRs are updated when behavior, architecture, or workflow change
<!-- DOD:END -->
