---
id: CSP-450
title: Operator migration notes
status: Done
assignee: []
created_date: '2026-06-22 21:57'
labels:
  - p11
milestone: m-17
dependencies: []
ordinal: 514000
---

## Final Summary

<!-- SECTION:FINAL_SUMMARY:BEGIN -->
New top-level `CHANGELOG.md` (Keep a Changelog
1.1.0 format) with an `[Unreleased]` section that lists
Phase 11's breaking changes (Removed: `conspectus query`,
vector-search, `graph.sqlite` persistence layer), the
additions (`graph.bin`, `snapshot` socket command,
daemon warm-restart, legacy-cache cleanup), the changed
behaviors (refresh/status flows now route through the
socket; `--no-cache` / `--refresh` semantics preserved),
and the unchanged surfaces (TOML and provider-owned
SQLite usage). Cross-links to the operator migration
section in `docs/operations.md`.
`docs/operations.md` gains a `## Migration from earlier
0.x` section at the tail covering:
- Cache file change (`graph.sqlite{,-wal,-shm}` +
  `backups/` → `graph.bin`) and the one-shot daemon
  cleanup behavior, with an `rm -rf` recipe for
  operators who haven't started a daemon yet.
- `conspectus query` removal + the `conspectus graph
  --format json | jq` migration recipe + saved-view
  replacement notes.
- Vector-search retirement.
- `snapshot` socket command addition (transparent to
  operators).
- User-authored TOML / pins / aliases unaffected.
- Daemonless cold rebuild as the new floor; recommend
  `conspectus serve` for sub-second CLI response.
The existing §"Caches" section rewritten to describe
both persistent cache surfaces (`graph.bin` + pin-
binding sidecar) as parallel rebuildable artifacts.
Two pre-existing stale references to `conspectus query`
in the TUI-state read-only-invariant paragraph dropped.
Tests: docs-only; `git diff --check` clean.
<!-- SECTION:FINAL_SUMMARY:END -->

Legacy ID: `P11-013`
