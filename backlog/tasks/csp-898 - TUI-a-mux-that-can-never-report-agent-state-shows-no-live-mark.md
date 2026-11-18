---
id: CSP-898
title: 'TUI: a mux that can never report agent state shows no live mark'
status: Done
assignee:
  - '@claude'
created_date: '2026-11-18 00:43'
updated_date: '2026-11-18 01:17'
labels:
  - tui
dependencies: []
ordinal: 974000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
A mux with no agent harness and no session linkage (the daemon's own conspectus-server, a plain shell) draws the no-state mark in its live cell, as if its state were unknown or its hooks off. It can never have reportable state, so the mark is noise; the operator asked for no symbol at all there.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [x] #1 A mux row whose pane runs no agent harness and that no session or agent counts toward draws a blank live cell, in the Mux view (running and stopped rows alike), with the column grid unchanged
- [x] #2 A mux that has an agent, a harness program in a pane, or any live evidence keeps its mark, the no-state mark included
- [x] #3 A session row's live cell is unchanged
- [x] #4 docs/tui-columns.md and the help legend say when the no-state mark shows; tests cover both cases
<!-- AC:END -->

## Definition of Done
<!-- DOD:BEGIN -->
- [x] #1 `just check` passes, or `git diff --check` for docs-only changes
- [x] #2 Docs and ADRs are updated when behavior, architecture, or workflow change
<!-- DOD:END -->

## Final Summary

<!-- SECTION:FINAL_SUMMARY:BEGIN -->
A mux row that can never report agent state (no session linked, no agent counted even an exited one, no harness in its pane, and a stopped mux whose last run had no agent) draws a blank live cell instead of the no-state `·`; the column keeps its width. A mux that has or may have an agent keeps the mark, and session rows are unchanged (`MuxSessionRow::never_reports_state`, ADR 0249). Verified: consistency tests for a shell-only and a server pane (blank), a harness pane and a linked agent and a pane agent's live state (mark kept), a stopped mux with and without an agent chip, and a session with no state (mark kept); the live and column-grid parsers now find the mark by the state glyph after it; the real TUI against a private tmux server showed `conspectus-server` and a plain shell session with no mark and `no agent`. docs/tui-columns.md and the help legend say when it shows. Pre-main gate: just check, separation rehearsal, just plugins, nextest with no conspectus on PATH passed; cargo test passed on the rerun after two unrelated timing and tmux-start flakes (a header `scanned 1s ago` against `0s ago`, and the pasted rebuild script's tmux start) passed alone.
<!-- SECTION:FINAL_SUMMARY:END -->
