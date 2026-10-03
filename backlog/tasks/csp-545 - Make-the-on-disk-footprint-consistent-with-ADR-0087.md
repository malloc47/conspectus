---
id: CSP-545
title: Make the on-disk footprint consistent with ADR 0087
status: To Do
assignee: []
created_date: '2026-09-30 18:30'
labels:
  - rel
milestone: m-20
dependencies: []
priority: medium
ordinal: 616000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
- Scope: ADR 0087 and `AGENTS.md` say rebuildable sidecars live under
  `$XDG_STATE_HOME/conspectus/`, but pin-binding sidecars live under
  `$XDG_CACHE_HOME/conspectus/pin-bindings/` and `graph.bin` under
  `$XDG_DATA_HOME/conspectus/`. `graph_bin_path()`
  (`src/snapshot.rs:74`) falls back to `./conspectus/graph.bin` in the
  cwd when neither `$XDG_DATA_HOME` nor `$HOME` is set, which can write
  inside a project tree. Amend ADR 0087 to describe the real layout (or
  move files), fix the fallback, and add one "Files Conspectus writes"
  table to `docs/operations.md`.
- Blockers: none.
<!-- SECTION:DESCRIPTION:END -->

Legacy ID: `REL-014`
