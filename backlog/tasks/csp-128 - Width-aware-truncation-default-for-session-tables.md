---
id: CSP-128
title: Width-aware truncation default for session tables
status: Done
assignee: []
created_date: '2026-05-18 14:50'
labels:
  - h-tbl
milestone: m-11
dependencies: []
ordinal: 120000
---

## Final Summary

<!-- SECTION:FINAL_SUMMARY:BEGIN -->
`src/output/table.rs` now exposes `RenderOptions { width,
layout }` and `render_with(snapshot, projection, options)`. The
existing `render(...)` is preserved as a thin wrapper around
`RenderOptions::wide()`, so every snapshot test stayed byte-for-byte
stable. `render_with` measures every cell via
`unicode_width::UnicodeWidthStr::width`, greedy-shrinks per-column
budgets toward the target width (never below `max(header_width, 4)`),
and truncates overflowing cells with a trailing `…`. The `session`
subcommand gained `--wide` and `--width <N>` flags. Default behavior:
`--wide` ⇒ untruncated; `--width N` ⇒ exact N columns; otherwise
detect via `terminal_size::terminal_size()` when stdout is a TTY,
else stay wide so pipes remain grep/awk-friendly. Added unit tests
for truncation/budget edge cases (including wide CJK columns) and
CLI integration tests for `--wide`, `--width`, the pipe-stays-wide
default, and the clap-level `--wide`/`--width` conflict.
Dependencies recorded by ADR 0020: `unicode-width = "0.2"` and
`terminal_size = "0.4"`.
<!-- SECTION:FINAL_SUMMARY:END -->

Legacy ID: `H-TBL-003`
