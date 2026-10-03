---
id: CSP-194
title: >-
  Round-trip attach: return to the TUI after the operator detaches from the mux
  client
status: Done
assignee: []
created_date: '2026-05-20 13:38'
labels:
  - t8
milestone: m-13
dependencies:
  - CSP-169
ordinal: 441000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
- Scope: today `exec_tmux_attach` calls `execve`, so the
  conspectus process is replaced by tmux. When the operator
  detaches (Ctrl-B d), there's no TUI to return to — they
  land at the parent shell prompt. Change the attach path to
  `Command::new("tmux").status()` (spawn + wait) under a
  suspend-resume guard: leave the alt screen + raw mode
  before spawning, restore them after wait, and feed a
  refresh into the reducer so the row tree reflects any
  activity that happened during the attach. Quit (`q`) from
  the TUI should still exit cleanly, and a failed spawn
  should land in the status bar with a clear reason instead
  of killing the process.
- Tests: extend the action tests to cover an `Action::Attach`
  that runs a fake "attach" closure and returns control to
  the reducer; assert the TUI is still alive afterward, that
  the next refresh is scheduled, and that a fake "command
  failed" surfaces as a status message. Manual: attach,
  detach, repeat from a different row.
- Blockers: `CSP-169` v1 slice.
<!-- SECTION:DESCRIPTION:END -->

## Final Summary

<!-- SECTION:FINAL_SUMMARY:BEGIN -->
`attach_action` no longer `exec`s into tmux.
Instead, it calls `ratatui::restore()`, spawns
`tmux attach-session -t <native_id>` with
`Command::status()` so tmux owns the real terminal, waits
for it to exit, then calls `ratatui::init()` to re-enter
the alt screen and replaces the runtime's
`DefaultTerminal` in place (followed by a `clear()`). Once
control returns, the runtime kicks off a refresh so the
row tree reflects activity during the attach, and surfaces
a status-bar line — `attached/detached: tmux:<name>` on
success or `attach failed: <reason>` if tmux exited
non-zero or didn't launch. The operator stays in the TUI
ready to pick another row.
<!-- SECTION:FINAL_SUMMARY:END -->

Legacy ID: `T8-018`
