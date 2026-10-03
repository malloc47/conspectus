---
id: CSP-241
title: TUI `R` keybinding wires rename flow
status: Done
assignee: []
created_date: '2026-05-23 02:59'
labels:
  - h-rename
milestone: m-11
dependencies:
  - CSP-237
  - CSP-240
ordinal: 290000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
- Scope: bind `R` (capital) — verify it's unused today
  (`src/tui/runtime.rs:324-325`). On press, opens the input widget
  pre-populated with the current alias (or harness title, or empty
  when neither). Enter triggers `CSP-239` plan → alias write +
  optional `TmuxRunner::rename_session` → `Msg::SetStatus` feedback
  (`renamed: <new>` or `rename failed: <reason>`) → refresh. Esc
  cancels. Lower-case `r` continues to mean refresh per Phase 8.
- Tests: reducer tests for the open/confirm/cancel paths. Insta
  snapshot for the active rename overlay over the sessions tree.
  Manual: rename a session in a real TUI, confirm both alias and
  tmux update.
- Blockers: `CSP-237`, `CSP-240`.
<!-- SECTION:DESCRIPTION:END -->

Legacy ID: `H-RENAME-011`
