---
id: CSP-189
title: Default-expand and mark the launch-context project without filtering the world
status: Done
assignee: []
created_date: '2026-05-20 03:28'
labels:
  - t8
milestone: m-13
dependencies: []
ordinal: 436000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
- **slice landed**: initial expansion now opens only the
  launch-context tree (ancestors, the marked group, and its
  descendant groups) while leaving unrelated groups collapsed
  for a cleaner first screen. If no launch-context group is
  marked, the first group opens as a fallback so tests and
  non-cwd-oriented data still present a usable starting point.
- Tests: `cargo test tui --all-targets`.
<!-- SECTION:DESCRIPTION:END -->

## Final Summary

<!-- SECTION:FINAL_SUMMARY:BEGIN -->
Scan roots now resolve CLI → `[tui].scan_roots`
config → cwd-default (operator picked option (b)).
`src/config.rs` gained a `TuiConfig` struct with
`scan_roots: Vec<PathBuf>` parsed from `[tui].scan_roots` in
`.conspectus.toml` / user config, with `~` and `~/<rel>`
expansion against the loader's home directory at merge
time. `TuiArgs::run` consults the loaded config when
`--scan-root` is empty and falls back to `[cwd]` only when
both are unset. `RunConfig` carries the launch-time cwd as
an orientation hint that the sessions row-tree builder
uses to mark the deepest ancestor group row with
`GroupRow::is_launch_context = true`. `Msg::SetData`
carries an `initial_selection_hint`; the runtime computes
it from the marked row and the reducer prefers it on first
load over the leading row (later refreshes ignore the
hint so manual selection isn't clobbered). The renderer
adds a dim cyan `(cwd)` suffix to the marked row. Auto-
broaden option (c) filed as low-priority `CSP-196`.
<!-- SECTION:FINAL_SUMMARY:END -->

Legacy ID: `T8-013`
