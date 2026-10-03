---
id: CSP-166
title: Render the two-panel Ratatui UI
status: Done
assignee: []
created_date: '2026-05-19 03:51'
labels:
  - p8
milestone: m-13
dependencies:
  - CSP-162
  - CSP-163
  - CSP-164
ordinal: 393000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
- Scope: implement the visible layout per the wireframe and panel
  composition in `docs/implementation/phase-08-interactive-tui.md`:
  50/50 left/right split at wide widths, stacked layout below
  ~100 columns, single-line status bar with action hints on the
  left and provider status chips on the right. Right panel is
  fixed header + fixed preview (no tabs in v1). Implement the
  distinct empty/loading/error frames described in the
  "Empty, Loading, And Error States" table — including the
  "discovery in flight" placeholder, the "no sessions discovered"
  empty graph, the `tmux disabled` / `tmux unavailable` mux-view
  fallbacks, the `--no-live-preview` preview-zone message, the
  refresh-failure stale marker, and the selection-snap-on-removed-
  row behavior. Use fixed-dimension Ratatui buffer snapshots for
  desktop-ish and narrow terminal sizes; truncate/wrap text
  coherently, show row depth and selected state, and avoid
  blocking on discovery or preview capture.
- Tests: Ratatui buffer snapshot tests for sessions, mux, PR,
  narrow terminal, search overlay, help overlay, provider-error
  status, empty graph, `--no-live-preview`, and selection
  retention after a refresh that removes the selected row. Keep
  snapshots deterministic by using fixture graphs and fixed
  terminal sizes.
- Manual checks: `cargo run -- tui --view sessions`,
  `cargo run -- tui --view mux`, `cargo run -- tui --no-live-preview`,
  and terminal resize while running.
- Blockers: `CSP-162`; friendlier after `CSP-163` and `CSP-164`.
- **v1 slice landed**: two-panel render with header bar, left
  row tree (depth-indented disclosure glyphs, mux indicator
  glyph with color, same-line session previews when width
  allows), right detail (title line + header fields + preview
  block), status bar. Empty-/loading-frame placeholders cover
  the no-data case. Remaining work for full CSP-166 (filed as
  follow-ons):
  - `CSP-180`: full empty/loading/error frame matrix per the
    phase-08 "Empty, Loading, And Error States" table —
    `--no-live-preview` zone message, tmux-unavailable banner,
    provider-error chips, refresh-failed stale marker.
  - `CSP-181`: responsive layout — narrow-terminal stacked
    panels (< 100 cols) plus same-line row preview behavior.
  - `CSP-182`: `updated Ns ago` header indicator (requires
    `loaded_at_epoch` on `Msg::SetData` and `App`).
  - `CSP-183`: snapshot test coverage matrix beyond the v1
    sessions-render smoke test (mux/PR/narrow/overlays/empty/
    error/selection-retention).
<!-- SECTION:DESCRIPTION:END -->

## Final Summary

<!-- SECTION:FINAL_SUMMARY:BEGIN -->
Outcome (2026-09-30, `CSP-538`): v1 landed in `bc7a1b0`; the
empty/loading/error frame matrix continues as `CSP-180`.
<!-- SECTION:FINAL_SUMMARY:END -->

Legacy ID: `P8-007`
