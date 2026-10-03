---
id: CSP-087
title: Split `src/declared.rs` by concern
status: Done
assignee: []
created_date: '2026-05-17 20:45'
labels:
  - h-ref
milestone: m-11
dependencies:
  - CSP-083
ordinal: 87000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
- Landed 2026-07-05 (`f234df5`). Directory-based module
  with three submodules: `mod.rs` (TOML types + endpoint
  codec + parse/validate + error types, 413 lines),
  `store.rs` (read-modify-write helpers + store types +
  file I/O, 253 lines), `snapshot.rs` (graph-driven
  decision helpers, 251 lines). Every pre-H-REF-005
  `crate::declared::*` public identifier re-exported from
  `mod.rs` so callers compile unchanged. `write_atomic`
  kept as `pub(crate)` at the module path because pins,
  aliases, and tui_state share it. Test module moved
  into the new directory as `tests.rs`.
- Blockers: `CSP-083` (landed).
- Scope: separate (a) TOML models + parse/validate, (b) read-modify-write
  helpers and file I/O, and (c) snapshot-aware helpers
  (`endpoint_project_root`, `declared_endpoint_from_node_id`,
  `select_store_for_declaration`). The last group reaches into a
  `GraphSnapshot` and should live near discovery, not next to the file
  format.
- Tests: existing declared and CLI tests must continue to pass without
  snapshot diffs.
- Blockers: `CSP-083` is friendlier to do first.
<!-- SECTION:DESCRIPTION:END -->

Legacy ID: `H-REF-005`
