---
id: CSP-461
title: Project pins as placeholder session and mux entities
status: Done
assignee: []
created_date: '2026-06-26 20:48'
labels:
  - h-pin-tui
milestone: m-11
dependencies: []
ordinal: 329000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
- Motivation: creating a new, unlaunched pin from the mux view can
  currently leave no mux-shaped row to select because no real
  `MuxSessionNode` exists yet. A pin is a first-class declaration,
  but it also describes the user's intended next mux/session, so the
  session and mux views need placeholder entities that preserve the
  user's current view vocabulary before first launch.
- Scope: decide the graph shape for pin-introduced placeholders and
  implement it consistently across the resolver, row builders,
  details, search, and launch/attach affordances. Preserve
  `PinNode` as the authored declaration and store-lineage entity,
  while allowing unbound pins to contribute placeholder
  `AgentSession`- and `MuxSession`-shaped rows keyed by stable
  identities derived from the pin. In grouped session/mux views,
  the Pins bucket should contain session-shaped rows in the sessions
  view and mux-shaped rows in the mux view; in flat views, these
  placeholders should sort with pinned entities near the top. When
  a real mux/session appears, the placeholder should resolve into
  the observed node without changing the operator's row-level mental
  model. Include the last-resolved session sidecar as the preferred
  placeholder session identity when available.
- Tests: model/resolver tests for placeholder identity stability,
  unbound-to-bound transition, stale-mux behavior, and sidecar-backed
  last-session projection; row-builder tests for sessions and mux
  views across grouped and flat groupings; TUI snapshot coverage for
  an unlaunched pin created from mux view; detail/search tests that
  show the placeholder row links back to the `PinNode`.
- Progress: first implementation slice uses row-only placeholders:
  unbound pins emit session-shaped rows in the sessions view and
  mux-shaped rows in the mux view, with `primary_node` pointing at
  the owning `PinNode`, pin launch as the default action, and a
  `planned` visual marker. Detail/search/model placeholder identity
  work remains open under this story.
- Decision note: the first slice uses row-only view models backed by
  the existing `PinNode`; a later slice can still promote placeholder
  identities into graph nodes if detail/search/JSON consumers need
  that surface.
- Styling polish (2026-06-26): dropped the "planned" vocabulary
  from placeholder rows entirely. Both the mux and session
  placeholder rows render the attached-glyph column as a colored
  `◌` (U+25CC dotted circle) aligned with `◉ / ◯ / ?` on real rows,
  using a new `theme.pin_placeholder` color (default
  `Color::LightYellow`) so the glyph stands out against the dim
  row body. Single-session preview / session preview fall through
  to the pin's cwd (`~`-collapsed). Session placeholder rows also
  drop the trailing "planned" chip and the "planned" title.
- Repo-bucket placement (2026-06-27): unbound / stale-mux pin
  placeholders now also surface under the project bucket whose
  checkout contains the pin's cwd, mirroring how bound pinned
  sessions appear in both the synthetic Pins group at top *and*
  their natural project bucket. The row builder adds a `pin_group_
  key` helper that mirrors `resolve_group_key`'s repo path,
  pre-seeds buckets for placeholder-only keys so empty project
  headers still emit, and emits placeholder rows after sessions at
  the same depth. Pins whose cwd doesn't map to any checkout
  continue to appear only in the Pins group.
- Detail-pane dispatch (2026-06-27): placeholder pin rows route
  through a view-aligned upgrade in `App::recompute_detail`. The
  sessions view upgrades to the `PinUnbound` diagnostic's
  `last_session` when its `AgentSession` node is present in the
  snapshot; the mux view upgrades to the bound / stale-mux target
  when the pin has one. When neither is available the right pane
  falls back to the `PinNode` itself, with both `NodeDetail`
  candidate-link summaries and the `NodeView` Related zone stripped
  so the surface reads as "no live entity yet" rather than dumping
  the resolver-synthesized `LinkedToMux` candidate. Header label
  now reads `pin` (the missing arm in `right_panel_kind_label` was
  added in the same pass).
- Remaining work: bound-pin `📌` glyph polish stays deferred to
  `CSP-376`; search / model / JSON placeholder identity is still
  open under this story.
<!-- SECTION:DESCRIPTION:END -->

## Final Summary

<!-- SECTION:FINAL_SUMMARY:BEGIN -->
Outcome (2026-09-30, `CSP-538`): done per operator review: WIP
`719d77a`, then `8870e82`, `fc55193`, `88450d8`, and `0524fdd` finished
the polish.
<!-- SECTION:FINAL_SUMMARY:END -->

Legacy ID: `H-PIN-TUI-010`
