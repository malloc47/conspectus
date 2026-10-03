---
id: CSP-340
title: 'Viewer-bridge integration: wire the `T` keybind into the native viewer'
status: Done
assignee: []
created_date: '2026-06-02 18:59'
labels:
  - h-viewer-native
milestone: m-11
dependencies: []
ordinal: 215000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
- App carries `viewer_modal: Option<ViewerState>` with
  `open_viewer_modal` / `close_viewer_modal` /
  `take_viewer_modal` accessors. The runtime adds
  `Action::ViewerOverlayKey(KeyEvent)` and makes the
  viewer-modal check the **highest-priority** overlay (above
  value_modal etc.) so all keys route through the modal while
  it's open.
- `view_action` rewritten: pull the selected `AgentSessionId`,
  call `viewer_bridge::build_viewer_state`, and
  `app.open_viewer_modal(state)`. Falls through to the
  pre-existing escape-hatch external launcher
  (`ClaudeHistoryViewer`) only when the harness has no
  native parser (currently: `aider`). The minimal
  "external is opt-in" sketch from the story — full
  `[viewers.<harness>]` config lands with
  `CSP-332`; until then native is unconditional for
  every supported harness.
- `handle_viewer_overlay_key` translates crossterm keys into
  `ViewerMsg` and runs the pure reducer (`take`-reduce-`put`
  pattern around `App::take_viewer_modal`). Keymap:
  `q`/`Esc`/`Ctrl-C` → Close, `j`/`Down` → ScrollDown,
  `k`/`Up` → ScrollUp, `PgDn`/`Space` → PageDown,
  `PgUp` → PageUp, `Ctrl-D`/`Ctrl-U` → half-page,
  `g`/`Home` → JumpToStart, `G`/`End` → JumpToEnd,
  `t` → ToggleTools, `y` → ToggleThinking. Unknown keys
  drop silently.
- The TUI `draw` path branches on `viewer_modal_mut()`: when
  Some, the widget takes the full frame area; when None, the
  standard two-panel render proceeds unchanged. Mux-row
  selection behavior is unchanged — `T` only opens the modal
  for `AgentSession` row selections.
- Help overlay description updated:
  "Open the selected session's transcript (q/Esc close, j/k
  or PgDn/PgUp scroll, g/G start/end, t tools, y thinking)".
- 7 bridge tests (locator mapping for all three harnesses,
  OpenCode parent-dir vs. .db filename, unknown harness =
  None, `build_viewer_state` None for unsupported / fallback
  to unavailable doc when file missing). 1226 nextest green.
<!-- SECTION:DESCRIPTION:END -->

## Final Summary

<!-- SECTION:FINAL_SUMMARY:BEGIN -->
`src/tui/viewer_bridge.rs` translates an
`AgentSessionId` into the matching `SessionLocator` variant
(with OpenCode `state_scope` treated as either the parent dir
or the `.db` path itself), runs it through the right
`HarnessParser`, and wraps the result in a `ViewerState`.
Parser failures degrade to
`TranscriptDocument::unavailable(...)` so the widget always
has something coherent to render. The bridge lives outside
`src/viewer/` per ADR 0052 — it's the only file in conspectus
that knows both the graph-flavored `AgentSessionId` and the
viewer-flavored `SessionLocator`.
<!-- SECTION:FINAL_SUMMARY:END -->

Legacy ID: `H-VIEWER-NATIVE-008`
