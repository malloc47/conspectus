---
id: CSP-416
title: >-
  Weave per-node-kind glyph identity through every TUI surface (tree, detail,
  filter, help)
status: Done
assignee: []
created_date: '2026-06-17 15:15'
labels:
  - h-ui
milestone: m-11
dependencies:
  - CSP-408
ordinal: 356000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
- Scope: re-affirm and finish the existing
  `Per-Node-Type Visual Identity` workstream
  (`CSP-408..413`) — there are enough unique graph entity
  types (Workspace, Repo, Checkout, AgentSession, MuxSession,
  Branch, Fork, ForgePr, RuntimeProcess) that operators need a
  shorthand glyph per kind, not just a textual label. Beyond
  the row-tree + detail-panel scope already captured in
  `CSP-410..411`, extend the glyph usage to the help modal
  keybinding tables (where the modal references a node kind),
  the filter modal (kind-bucket headers and chip pills), the
  search results overlay, the breadcrumb chain in the detail
  explorer, and any non-TUI surface that names node kinds
  (CLI table rows, JSON `node_kind` tag per `CSP-412`).
  Acceptance under the existing `H-VIS-*` IDs; this story
  promotes the workstream from "candidate" to "scheduled."
- Slice landed (breadcrumb chain): `render_breadcrumb_chain`
  in `src/tui/explorer.rs` now returns a styled `Line` with
  one `<kind-glyph> <tag>` segment per hop instead of
  `kind:short_tag` text. The kind comes from `NodeKind::from(
  &hop.focused)` so the correct glyph + per-kind color land
  on every segment; the tag is the part after the `kind:` prefix
  in `BreadcrumbHop::short_label`, with the existing `·xxxx`
  disambiguation suffix preserved. Elision (full / first …
  last / only last / fallback) now measures display width across
  spans. The caller in `right_panel_title` pushes the chain's
  spans verbatim so the kind color survives. Tests cover the
  flat plain-text shape, the per-glyph kind color, the
  elision ladder, and the disambiguation tail.
- Slice landed (search results overlay): `build_match_line` in
  `src/tui/widgets/search.rs` now inserts a 2-cell kind glyph
  span (`<glyph> `) between the cursor prefix and the label,
  so operators scan results by symbol instead of relying on
  the textual `kind:` prefix some labels carry. The kind is
  derived from `RowId` via a small `search_row_node_kind`
  helper that covers Group (via NodeId), AgentSession,
  AgentSessionMuxCandidate (mux glyph), MuxSession, Pr, and
  Fork. Pin and Synthetic rows return `None` and the glyph
  span renders as two blank cells so the label column stays
  aligned across the result list. ForgePr's glyph falls back
  to `theme.pr_open` (same dodge as `kind_chip_span` and the
  breadcrumb renderer, since search results don't carry PR
  state). Tests cover the kind-color span shape for
  AgentSession + MuxSession, the two-space fallback for Pin
  and Synthetic, and an end-to-end `build_match_line`
  assertion that the agent-session glyph appears in the
  correct color before the label.
- Slice landed (help modal icon legend): `body_lines` in
  `src/tui/widgets/help.rs` gains a `Node kind icons (ADR 0073)`
  section that walks `NodeKind::ALL` and renders each entry as
  `<glyph> <display name> <one-line blurb>` so operators learn
  the symbol vocabulary by pressing `?` instead of cross-
  referencing the design docs. `node_kind_display_name`
  (operator-facing labels: `Agent session`, `Forge PR`, …) is
  kept distinct from the existing `theme_key` / `snake_case`
  accessors so the legend reads naturally. The keybinding rows
  that already mention kinds in prose are left alone — the
  legend covers the at-a-glance "what does this glyph mean"
  question without a more invasive refactor of help-text strings.
- Closed for the TUI scope. The filter modal does not actually
  carry NodeKind-bucket headers — its dimensions are harnesses,
  mux states, views, groupings, and sort, none of which map to
  NodeKinds — so the "filter modal kind-bucket headers and chip
  pills" item in the original scope had no real target. Non-TUI
  output surfaces (CLI table rows, JSON `node_kind` tag, DOT /
  HTML payloads) stay under `CSP-412`, which already owns them.
- Blockers: see `CSP-408`.
<!-- SECTION:DESCRIPTION:END -->

Legacy ID: `H-UI-002`
