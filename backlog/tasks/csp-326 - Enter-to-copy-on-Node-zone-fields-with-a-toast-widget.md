---
id: CSP-326
title: Enter-to-copy on Node-zone fields with a toast widget
status: Done
assignee: []
created_date: '2026-06-01 15:17'
labels:
  - t8
milestone: m-13
dependencies:
  - CSP-313
ordinal: 431000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
- Scope: `Enter` on a Node-zone field row is a no-op today.
  Wire it to copy the field's full value to the system
  clipboard and surface a transient toast ("copied: cwd") via a
  new reusable toast widget under `src/tui/widgets/` that
  auto-dismisses after ~1.5s and does not block input. This
  cleanly splits the contract: `o` for *reading* a long value
  (modal, scrollable), `Enter` for *copying*. Reuse the toast
  for `i` (copy short id) and any future copy actions so
  feedback is consistent. The `i` binding copies the full id of
  the selected agent or mux session (not a short id) so the
  output drops directly into `node show` / external tooling.
  Clipboard backend is OSC 52 per
  ADR 0056 (no new deps, SSH-friendly, hand-rolled escape
  writer at `src/tui/clipboard.rs`); `arboard` deferred until
  operator feedback shows the OSC 52 gap biting.
- Tests: reducer/keymap tests for Enter-on-Node-field copying
  the value and surfacing a toast; toast widget unit tests for
  auto-dismiss timing and replacement (newer toast supersedes
  older); regression test that Enter on link rows still drills
  and Enter on group headers still toggles; coverage that empty
  or absent values don't surface a misleading "copied" toast.
- Blockers: `CSP-313` (cleared). Clipboard backend ADR landed as
  ADR 0056.
- **slice landed**: OSC 52 clipboard primitive at
  `src/tui/clipboard.rs` (in-tree base64 encoder, no new deps per
  ADR 0056). Reusable `ToastWidget` at
  `src/tui/widgets/toast.rs` auto-dismisses after 1500ms and is
  rendered as a non-blocking bottom-centered overlay; newer toasts
  replace older. Right-pane `Enter` on a Node-zone field row now
  copies the field value (preferring the untruncated `long_value`
  when present) and posts a `copied: <label>` toast; link rows
  still drill, group headers still toggle. `i` copies the selected
  agent or mux session's full id (e.g.
  `agent_session:claude:proj_a:7d3f…`) via the same toast surface
  and surfaces a status hint when the selection isn't a session
  row. Help overlay advertises both bindings.
<!-- SECTION:DESCRIPTION:END -->

Legacy ID: `T8-040`
