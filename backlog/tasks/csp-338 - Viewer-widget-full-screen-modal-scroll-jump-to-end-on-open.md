---
id: CSP-338
title: 'Viewer widget: full-screen modal, scroll, jump-to-end-on-open'
status: Done
assignee: []
created_date: '2026-06-02 18:59'
labels:
  - h-viewer-native
milestone: m-11
dependencies: []
ordinal: 213000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
- Tests: 3 state, 9 reducer, 6 render, 7 widget — 25 total.
  Widget snapshot tests via insta cover (a) normal open at
  last turn, (b) JumpToStart on a 30-turn doc, (c) empty
  "transcript unavailable" doc, (d) compaction-summary turn
  rendered with banner header. Additional widget tests
  confirm `draw` writes viewport/total back to state, footer
  advertises current toggle state, and `ToggleTools` makes
  tool turns visible.
- Bridge wiring (`CSP-340`) still pending — `T`
  keybind continues to route through the escape-hatch
  `ClaudeHistoryViewer`. 1219 nextest green.
<!-- SECTION:DESCRIPTION:END -->

## Final Summary

<!-- SECTION:FINAL_SUMMARY:BEGIN -->
`src/viewer/{widget,state,input,render}.rs` ship the
full Ratatui modal. Layout: 1-line header
(`<harness>:<session-key>` left, `cwd: <cwd>` right-justified),
1-line `─` separator, flex body, 1-line bottom separator,
1-line footer with key hints. `state.rs::ViewerState`
carries the document, scroll offset, sticky-end flag, show
tools/thinking toggles, plus viewport-height +
total-line-count metrics written back by the draw fn so
the reducer has fresh layout numbers for page-down deltas.
`input.rs::reduce(state, msg) -> (state, ViewerEffect)` is
pure. Messages: ScrollUp/Down, PageUp/Down, HalfPageUp/Down,
JumpToStart, JumpToEnd, ToggleTools, ToggleThinking, Close.
All clear the sticky-end flag except JumpToEnd which sets
it; layout toggles re-clamp the scroll offset against
`max_scroll`. `render.rs::render_turn` emits a dimmed role
header (`you` / `assistant · thinking` / `— compaction
summary —` etc.), the body (Markdown via `tui-markdown` for
Message + CompactionSummary; dim plain text for Thinking /
ToolUse / ToolResult), and a spacer line. `widget.rs::draw`
short-circuits the empty-doc case with a "transcript
unavailable" banner; otherwise builds the flat body line
Vec (filtering tools/thinking by state flags) and scrolls
via `Paragraph::scroll`. Stick-to-end pins scroll to
`max_offset` on every draw until the operator manually
scrolls. Footer text adapts to current tool/thinking
state and truncates with `…` on narrow terminals.
<!-- SECTION:FINAL_SUMMARY:END -->

Legacy ID: `H-VIEWER-NATIVE-006`
