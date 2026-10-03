---
id: CSP-460
title: Promote pins to first-class graph entities
status: Done
assignee: []
created_date: '2026-06-25 19:23'
labels:
  - h-pin-tui
milestone: m-11
dependencies: []
ordinal: 328000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
- Scope: replace the sidecar-only `GraphSnapshot::pins` projection
  with a first-class graph entity for each declared pin while
  preserving the TOML schema as the persistence source. Pin nodes
  should have stable node ids, detail panes, searchable identity,
  source/store lineage back to the owning `.conspectus.toml` or
  user config, and graph relationships to the related mux,
  attributed agent session, cwd/workspace/repo context, and any
  launch/resume sidecar state. The Sessions view Pins tree should
  select a real graph entity, not a detail-less synthetic row; when
  a pin is bound, the pin detail should link to the realizing
  session rather than pretending the pin row is itself that
  session.
- Tests: model round-trip coverage for the new pin node/id,
  resolver tests for pin-to-mux/session/context relationships,
  detail tests for pin fields and store lineage, TUI row tests for
  selecting bound/stale/unbound pins, search tests, and graph JSON
  snapshots that expose pins as nodes plus links.
- Blockers: ADR/model decision for pin node identity and relation
  kinds.
<!-- SECTION:DESCRIPTION:END -->

## Final Summary

<!-- SECTION:FINAL_SUMMARY:BEGIN -->
Added ADR 0084 and promoted effective pin declarations
into derived `PinNode` graph entities with stable `pin:<id>` node
ids, store lineage fields, and resolver-copied binding state.
Resolved snapshots now synthesize `pin_targets_mux` links
(unresolved when the mux is absent) and `pin_realized_by_session`
links for bound pins. TUI pin rows now select `NodeId::Pin`, render
a pin detail pane with launch/store/binding fields, use the pin
glyph in rows/search/help, and link onward to the realizing session
or mux. Graph JSON snapshots expose pin nodes, pin links, and pin
node provenance; the rkyv snapshot cache format version was bumped
for the new enum variants.
<!-- SECTION:FINAL_SUMMARY:END -->

Legacy ID: `H-PIN-TUI-009`
