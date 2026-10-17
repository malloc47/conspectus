---
id: CSP-627
title: Drop the mux-row ambiguity glyph and add a mux-state legend to help
status: Done
assignee:
  - '@claude'
created_date: '2026-10-17 03:36'
updated_date: '2026-10-17 03:41'
labels:
  - h-ui
milestone: m-11
dependencies: []
priority: medium
type: enhancement
ordinal: 685000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Mux rows append a yellow `◐` after the attach-state glyph whenever any visible agent session under that mux has more than one mux candidate. Two problems were reported on 2026-10-16:

- The glyph is only drawn on some rows, so those rows are two cells wider and the pin marker and preview columns shift right. The mux list stops lining up.
- The operator has never acted on the glyph and couldn't remember what it meant. The `?` help modal only has a legend for node-kind icons, so none of the mux-state glyphs (`◉ ◯ ◌ ? ⚠`) are explained in the TUI.

The operator chose to remove the glyph from mux rows instead of reserving a column for it. Ambiguity is still shown by the group-row and header `⚠` (ADR 0072), the filter modal's ambiguous bucket, and the detail pane.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [x] #1 Mux rows in the mux and union views no longer render the ambiguity glyph, so the attach-state glyph, pin marker, and preview start in the same columns on every row
- [x] #2 The help modal has a mux-state legend explaining each glyph the TUI renders in the attach-state column and the ambiguity warning
- [x] #3 Tests cover the mux row without the ambiguity glyph and the help legend entries
- [x] #4 An ADR records dropping the mux-row ambiguity glyph, and docs that describe the glyph vocabulary are updated
<!-- AC:END -->

## Definition of Done
<!-- DOD:BEGIN -->
- [x] #1 `just check` passes, or `git diff --check` for docs-only changes
- [x] #2 Docs and ADRs are updated when behavior, architecture, or workflow change
<!-- DOD:END -->

## Implementation Plan

<!-- SECTION:PLAN:BEGIN -->
1. ADR 0120 (mux rows drop the ambiguity glyph), extending ADR 0072; index in docs/adr/README.md.
2. Remove the `◐` push in `left_panel.rs` and the now-unused `MuxSessionRow::ambiguous_count`.
3. Remove the `mux_ambiguous` theme key outright; a stale config gets the standard unknown-key diagnostic. Drop it from docs/operations.md and the pantry example.
4. Add a "Mux state" section to the help legend after "Node kind icons".
5. Tests: mux row alignment test, help legend test. Verify with `tui --snapshot --view mux` and the help modal snapshot.
<!-- SECTION:PLAN:END -->

## Final Summary

<!-- SECTION:FINAL_SUMMARY:BEGIN -->
Mux rows no longer append the yellow `◐` when a session under the mux has several mux candidates. The attach-state column is now one glyph wide in every state, so the pin marker and preview line up. `MuxSessionRow::ambiguous_count` and the `mux_ambiguous` theme key are removed, and `◐` no longer appears anywhere in the TUI. The `?` help modal has a new "Mux state" legend for `◉ ◯ ◌ ? ⚠`. ADR 0120 records the decision and extends ADR 0072. README, docs/operations.md, and docs/design.md are updated.

Verified with `just check` (fmt, clippy, 2319 nextest tests, rustdoc, diff check). New tests: `mux_rows_keep_pin_and_preview_columns_aligned_across_attach_states` and `help_body_includes_mux_state_legend`. The corroboration test now asserts the agent candidate count directly. Checked by eye with `tui --view mux --snapshot` on live data (the pin and preview columns line up) and with a `--snapshot-keys "?G"` help snapshot that shows the legend.
<!-- SECTION:FINAL_SUMMARY:END -->
