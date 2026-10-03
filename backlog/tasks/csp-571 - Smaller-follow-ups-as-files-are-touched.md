---
id: CSP-571
title: 'Smaller follow-ups, as files are touched'
status: Done
assignee: []
created_date: '2026-10-01 00:23'
labels:
  - h-rust
milestone: m-19
dependencies: []
ordinal: 580000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
- `cast_possible_truncation` and `cast_possible_wrap` (109 hits,
  mostly layout math): use `try_from` where a value can actually
  overflow.
- `items_after_statements` (25): move nested `use` and `fn` items to
  the top of the block.
- `&mut Option<Option<String>>` in the sessions row builder: use a
  named enum.
- The search overlay clones every visible row on each keystroke to
  escape a borrow.
- `push_str(&format!(..))` (21 sites): fine except on hot paths.
<!-- SECTION:DESCRIPTION:END -->

## Final Summary

<!-- SECTION:FINAL_SUMMARY:BEGIN -->
`items_after_statements` fixed (25 sites) and now denied.
The sessions builder's `Option<Option<String>>` became a
`PreviousHeader` enum. The search overlay no longer clones rows:
`items_from_rows` takes row references and returns owned items.
The casts were reviewed. Seventeen list-cursor steps moved to the
checked `tui::cursor::{wrap_step, clamp_step}` helpers, and
length-derived heights saturate with `u16::try_from`. The review found
two real bugs, both fixed with regression tests: the transcript viewer
wrapped its scroll offset past 65,535 lines (and cloned the whole
transcript every frame), and mux teardown would have passed a pid of
0 or -1 to `kill(2)`, signalling a process group. The remaining casts
are bounded (date math, wrap-width arithmetic, read-buffer sizes) and
stay. The ten `push_str(&format!(..))` sites in the TUI build short
labels once per render, so they stay as written.
<!-- SECTION:FINAL_SUMMARY:END -->

Legacy ID: `H-RUST-018`
