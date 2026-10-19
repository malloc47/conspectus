---
id: CSP-636
title: 'Show a spinner while a pin, mux new, or mux launch starts'
status: Done
assignee: []
created_date: '2026-10-19 18:19'
updated_date: '2026-10-19 18:26'
labels:
  - tui
dependencies: []
type: bug
ordinal: 694000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Starting an unstarted pin (and `mux new` / `mux launch`) leaves the screen blank or on the shell scrollback for seconds: the TUI is torn down for the `conspectus ... --no-attach` subprocess, then the blocking nudge-and-reload refresh runs before the attach. Nothing indicates progress. Keep the TUI drawn and show an animated status-bar spinner chip through both steps.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [x] #1 The TUI stays drawn, with an animated spinner chip, while the launch subprocess runs
- [x] #2 The same spinner covers the post-launch refresh before the attach
- [x] #3 pin launch, mux new, and mux launch share the behavior
<!-- AC:END -->

## Definition of Done
<!-- DOD:BEGIN -->
- [x] #1 `just check` passes, or `git diff --check` for docs-only changes
- [x] #2 Docs and ADRs are updated when behavior, architecture, or workflow change
<!-- DOD:END -->

## Final Summary

<!-- SECTION:FINAL_SUMMARY:BEGIN -->
Pin launch, `mux new`, and `mux launch` keep the TUI drawn with an animated status-bar spinner chip (`Launching <name>` / `Creating <name>`, then `Refreshing`) through the `--no-attach` subprocess and the refresh before the attach (ADR 0127, amending ADR 0108). Both steps run on a worker thread via `run_with_spinner`; the refresh is split into a worker-side fetch and a UI-side apply. Verified with `cargo fmt --check`, `cargo clippy -D warnings`, and the full nextest suite (2463 passed). New unit tests cover the chip lifecycle, the minimum-one-frame case, and worker-panic propagation. Not exercised against a live tmux in this session.
<!-- SECTION:FINAL_SUMMARY:END -->
