---
id: CSP-725
title: Extract `chip` and `age`
status: Done
assignee:
  - '@claude'
created_date: '2026-10-27 20:26'
updated_date: '2026-10-28 16:41'
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
- [x] #1 `tuikit::chip` provides the five chip forms and Conspectus's badges are built on it
- [x] #2 `tuikit::age` formats, buckets, and tints with injectable thresholds, and the four TUI formatters are removed
- [x] #3 The search overlay uses `theme.badge_width`
- [x] #4 The decision on sharing the age ladder with the CLI is recorded
- [x] #5 Every visible change is listed in the final summary under **Regressions** and **Consistency wins**, and was checked with `conspectus tui --snapshot` at a normal and a narrow width
- [x] #6 The framework crate names no Conspectus type, string, theme field, or domain word, and the independence guard passes
- [x] #7 The control has a reference page in the framework docs: purpose, usage, key table, theme tokens used, and the house rules it follows
<!-- AC:END -->

## Definition of Done
<!-- DOD:BEGIN -->
- [x] #1 `just check` passes, or `git diff --check` for docs-only changes
- [x] #2 Docs and ADRs are updated when behavior, architecture, or workflow change
<!-- DOD:END -->

## Implementation Plan

<!-- SECTION:PLAN:BEGIN -->
1. Capture before frames (left pane in every view, Search hit/pane rows, search overlay, viewers, header and status chips, explorer and Messages ages, Clean up) at 100 and 60 columns, with the default theme and badge_width 5 and 12, into /tmp/tuifw-baseline/frames-725-before.
2. `tuikit::age`: `Age` (seconds between two Unix times, clock skew clamped), `Ladder` with injectable units (`DAYS` s/m/h/d, the default; `WEEKS` adds w), `ago`, `Recency` with an injectable `RecencyScale` (5m/1h/1d) and its theme tint, and the right-aligned age `cell`. Boundary tests for every ladder step and recency step.
3. `tuikit::chip`: fixed-width filled `Chip` (theme badge look over a color, or a caller style; label width from `theme.badge_width`; cut and padded in cells; alignment), `ToggleChip`, `SegmentedTitle`, `PositionChip`, `StatusChip` with a `Tone`, and the spinner frame. Add `marks::SEPARATOR` for the middle dot.
4. Host: build `harness_badge`, `command_badge`, the `no agent`, `N hits`, and `was in` chips on the framework; the viewers' toggle and position chips, the search title's segments, and the header and status chips on the forms; the search overlay's chip column from the theme.
5. Host ages: remove `format_recency`, `recency_bucket`/`RecencyBucket`, `relative_epoch`, and `age_label`; rows, previews, header, Messages, explorer, and Clean up use `tuikit::age`; the Search view's hit, pane, and pane-hit rows draw the shared age cell; the CLI's `format_relative_age` uses the weeks ladder (output unchanged).
6. Docs: control pages chip.md and age.md, module map, README, tui-columns.md, ADR 0157 (chip width rule, the age ladder and the CLI decision), design.md, CHANGELOG, operator-review.
7. After frames, diff, checks (fmt, clippy, full isolated suite, doc, diff --check), backlog finalization.
<!-- SECTION:PLAN:END -->

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
- `format_elapsed` had already gone with the legacy detail header; the four removed are `format_recency`, `recency_bucket`/`RecencyBucket`, `relative_epoch`, and `age_label`. The Clean up list also borrowed the CLI's weeks formatter; it now uses the TUI ladder.
- CLI decision (ADR 0157 item 7): `format_relative_age` writes `Ladder::WEEKS` and keeps `now` for a future time; output unchanged, table tests unchanged.
- Frames: /tmp/tuifw-baseline/frames-725-{before,after} (every view, Search rows, overlay, viewers, header, status, explorer, Messages, Clean up at 100x30 and 60x24; default theme, harness chips on, badge_width 5 and 12; showcase and wide fixtures, 544 frames) and frames-725-{before,after}-wide (Search rows and overlay at 130 and 160 columns). Default theme: only the Search rows' age tint (style) and `scanned just now` -> `scanned 0s ago` differ. badge_width 5/12: the overlay chip column follows the theme. A rebuild at HEAD reproduced the after frames exactly.
- Commits split by concern; each intermediate state was checked (tuikit tests after the two framework commits; clippy and the host lib tests at the chip-only state).
<!-- SECTION:NOTES:END -->

## Final Summary

<!-- SECTION:FINAL_SUMMARY:BEGIN -->
Landed `tuikit::age` and `tuikit::chip` (ADR 0157) and moved the host onto them.

- `tuikit::age`: `Age` from two Unix times (clock skew is 0), `Ladder` (`DAYS` s/m/h/d by default, `WEEKS`, or host units), one `ago`, `Recency` with an injectable `RecencyScale` (5m/1h/1d) tinted from `recency_*`, and the four-cell `age::cell`. Boundary tests for every unit and recency step.
- `tuikit::chip`: the fixed-width filled `Chip` (`badge_width` in cells, cut with `…`, padding that carries the fill, theme `badge` over a caller color or a caller style, optional alignment), `ToggleChip`, `SegmentedTitle`, `PositionChip`, `StatusChip` with `Tone` and the busy spinner; `marks::SEPARATOR`.
- Host: `harness_badge`, `command_badge`, `no_agent_chip`, `hits_chip`, `was_in_chip` are thin adapters in `widgets/badge.rs`; viewers, search title, and status-bar chips use the forms; the content search's chip column is `chip::width(theme)`. `format_recency`, `recency_bucket`/`RecencyBucket`, `relative_epoch`, and `age_label` are removed.
- CLI: `format_relative_age` shares the formatter with weeks as its top unit; output unchanged (ADR 0157 item 7).
- Pages `crates/tuikit/docs/controls/chip.md` and `age.md`; tui-columns, design, changelog, ADR 0149 amendment, operator review.

**Regressions**
- Search status: `scanned just now` reads `scanned 0s ago`, and a day-old scan `1d ago` (was `25h ago`).
- Clean up: `last seen 3w ago` reads `last seen 21d ago` (days, like the History rows).
- Search view match, pane, and pane-line rows: the age is tinted by recency (fresh is bright green bold, a day or older dim) instead of gray.

**Consistency wins**
- One chip width rule: with a non-default `badge_width` the content search's chip column and the ` no agent ` chip follow it, so labels line up and the snippet stays inside the frame. Default unchanged.
- One age ladder and one `ago` across the TUI; every left-pane row draws the same age cell.
- Every other default frame is byte-identical: left pane in all views, overlay, viewers, header, status bar, explorer, Messages, Clean up, at 160, 130, 100, and 60 columns.

Verified: fmt, clippy (workspace, all targets, all features, -D warnings), full isolated suite (3742 passed, 0 failed, 1 ignored), nextest, rustdoc with -D warnings, `git diff --check`, the independence guard, the control-page examples compiled once as tests, and before/after `--snapshot` frames in /tmp/tuifw-baseline/frames-725-*.
<!-- SECTION:FINAL_SUMMARY:END -->
