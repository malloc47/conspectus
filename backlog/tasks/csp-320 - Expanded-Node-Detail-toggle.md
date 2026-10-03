---
id: CSP-320
title: Expanded Node Detail toggle
status: Done
assignee: []
created_date: '2026-06-01 03:32'
labels:
  - t8
milestone: m-13
dependencies:
  - CSP-313
ordinal: 425000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
- Scope: add a "full node" toggle that swaps the Node zone's
  top-5 render for every field the focused node carries
  (per `docs/tui-detail-mockup.md`'s Expanded Node Detail View
  section and the per-kind fields-reference tables). Default
  accelerator `F`; primary surface is the Controls overlay
  (ADR 0031). Toggle state is per-focused-node and resets when
  drilling into a neighbor; Backspace restores the prior node's
  toggle state along with its focus. Long values reuse the
  existing `(truncated · o)` open-value path. For node kinds
  whose available field set already fits in the top-5
  (`Repo`, `Workspace`, `Branch`, `Checkout`, `Fork`), the
  toggle is a no-op and renders the same content.
- Tests: reducer/keymap tests for toggle on/off across drills
  and backspace; renderer tests for each node kind's expanded
  field set; snapshot coverage for at least one expanded
  `agent_session`, `mux_session`, `runtime_process`, and
  `forge_pr` case.
- Blockers: `CSP-313` modeling.
<!-- SECTION:DESCRIPTION:END -->

Legacy ID: `T8-034`
