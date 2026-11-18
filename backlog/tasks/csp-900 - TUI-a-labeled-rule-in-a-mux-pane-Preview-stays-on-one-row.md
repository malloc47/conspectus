---
id: CSP-900
title: 'TUI: a labeled rule in a mux pane Preview stays on one row'
status: Done
assignee:
  - '@claude'
created_date: '2026-11-18 10:45'
updated_date: '2026-11-18 11:09'
labels:
  - tui
dependencies: []
ordinal: 976000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
The operator (2026-11-18) still sees Claude Code's prompt rules wrap onto several rows in a mux pane's Preview under the default smart wrap. Claude Code writes the session's name near the right end of the rule (`───…─── claude ─`, 155 columns). Smart wrap only cuts a line when everything past the Preview's edge is decoration or blank, so the label in the overflow counts as content. The line has no padding to squeeze either, so it word-wraps, and the unbroken run of `─` breaks into three rows of rule fragments. Other full-width banners with a label (pytest's `===== 3 passed in 0.2s =====`) wrap the same way. Smart wrap lives in tuikit's `richtext::layout_capture` (host ADR 0106, framework record 0058).
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [x] #1 Under smart wrap, a line whose overflow carries a label between long runs of one rule character fits on one row with the label kept, its rule runs shortened
- [x] #2 Content with short dash or equals runs (`--flag`, `===`, `->`) is never shortened; such a line wraps as before
- [x] #3 A rule whose overflow is only decoration is still cut at the edge, padding squeezes as before, and plain and none modes are unchanged
- [x] #4 A host ADR amending ADR 0106 and a framework record are added in the same change; the richtext page, operations guide, and design doc describe the behavior
- [x] #5 Tests cover the labeled rule, a label at either end and in the middle, short content runs, and a line that cannot fit even with its rules shortened
<!-- AC:END -->

## Definition of Done
<!-- DOD:BEGIN -->
- [x] #1 `just check` passes, or `git diff --check` for docs-only changes
- [x] #2 Docs and ADRs are updated when behavior, architecture, or workflow change
<!-- DOD:END -->

## Implementation Plan

<!-- SECTION:PLAN:BEGIN -->
1. Tests first in crates/tuikit/src/richtext/capture_tests.rs: Claude Code's labeled rule at 155 columns into 60, a label in the middle (pytest banner), a label at the left end (still cut), short content runs (`--flag`, `===`, `->`) wrapping as plain does, a label wider than the area wrapping as before, a two-style bar keeping its proportions, and the every-row-fits sweep with a labeled rule.
2. In capture.rs, after padding squeeze fails, shorten runs of four or more cells of one single-width decoration character, widest first, each to no fewer than one cell, keeping evenly spaced cells so a run of two styles keeps its proportions. Share the run finder and the shortening with the padding squeeze. None when the runs cannot give enough, so the line wraps as before.
3. Host ADR 0251 amending 0106, framework-owned; framework record 0058 corrected in place with 0251 in its Origin (the log's practice for an amendment), both index rows, ADR 0106 status, richtext page, architecture.md row, operations.md, design.md, CHANGELOG.
4. Run tuikit tests, just check, just tuikit-features, separation rehearsal; snapshot the mux Preview on a real Claude Code capture.
<!-- SECTION:PLAN:END -->

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
Snapshotting a real Claude Code capture showed a second cause, filed as CSP-901: since CSP-899 the Preview scrolls the capture beside a scrollbar but laid it out at the full width, so every full-width row (each cut rule) broke a lone cell onto its own row. Fixed there.

Validation: `just check` passes (5937 tests, tuikit-features, clippy, rustdoc, diff check). The tuikit vocabulary guard rejected `claude` in the framework's tests; they label the rule `notes`. Snapshot of the mux Preview on a read-only capture of a live Claude Code pane (155-column labeled prompt rule): before, three rows; after, one row ending `claude ─`, at 150 and 110 columns.
<!-- SECTION:NOTES:END -->

## Final Summary

<!-- SECTION:FINAL_SUMMARY:BEGIN -->
Smart wrap (`tuikit::richtext::layout_capture`) now shortens a line's rules, runs of at least four cells of one decoration character, widest first and to no fewer than one cell each, when a label past the area's edge keeps the line from being cut and squeezing its padding cannot fit it. Claude Code's prompt rule with the session's name at its right end takes one row and keeps the name, and a pytest-style banner keeps its words between shorter rules. Shorter runs (`--flag`, `===`, `->`) keep their length; a shortened rule keeps evenly spaced cells, so a two-style progress bar keeps its proportions; a label wider than the area still wraps. The padding squeeze and the rule shortening share one run finder and one shortener. ADR 0251 (framework-owned) amends ADR 0106; framework record 0058, both indexes, the richtext page, architecture.md, operations.md, design.md, and CHANGELOG describe it. Verified by seven new capture tests (labeled rule, centered banner, label before the edge, short content runs, oversized label, two-style bar, every-row-fits sweep), `just check`, and a snapshot of a real Claude Code capture.
<!-- SECTION:FINAL_SUMMARY:END -->
