---
id: CSP-634
title: Stamp the codex log query cache with the database's WAL file
status: Done
assignee:
  - '@claude'
created_date: '2026-10-18 06:41'
updated_date: '2026-10-18 11:53'
labels:
  - h-hyg
milestone: m-11
dependencies: []
references:
  - docs/adr/0125-opencode-2-reference-findings.md
priority: medium
type: bug
ordinal: 692000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
The codex log reader caches its thread observations keyed on `FileStamp::of(db_path)` (`src/discovery/codex_log.rs:361,369`). Codex's logs database runs in WAL mode, so a commit lands in `<db>-wal` and leaves the main file's mtime and length unchanged until a checkpoint. A warm TUI or daemon can then keep replaying observations that predate recent log rows. The same bug hid new opencode 2.x sessions; CSP-603 fixed it there with `memo::SqliteStamp`, which stamps the main file and its `-wal`. Found while fixing CSP-603.

- Scope: key the codex log query cache on `SqliteStamp`; check the other SQLite-backed caches for the same pattern.
- Tests: a cache test that commits a log row into the WAL (writer kept open, autocheckpoint off) and sees it on the next query; it fails with the main-file stamp alone.
- Blockers: none.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [x] #1 A codex log row committed only to the WAL is visible to the next cached query
- [x] #2 Every SQLite-backed discovery cache keys on the database and its WAL
<!-- AC:END -->

## Definition of Done
<!-- DOD:BEGIN -->
- [x] #1 `just check` passes, or `git diff --check` for docs-only changes
- [x] #2 Docs and ADRs are updated when behavior, architecture, or workflow change
<!-- DOD:END -->

## Implementation Plan

<!-- SECTION:PLAN:BEGIN -->
1. Survey SQLite-backed discovery caches for main-file-only stamps.
2. Key the codex log query cache on memo::SqliteStamp.
3. Add a WAL-commit cache test and confirm it fails with the old key.
<!-- SECTION:PLAN:END -->

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
Survey of SQLite readers: the codex log reader (`discovery::codex_log`) is the only one besides opencode's that caches a query result. The codex state DB reader (`harness/codex.rs` thread_spawn_edges) and agent-deck's reader query fresh every time, and the claude, codex-rollout, and pi caches stamp JSONL files, which have no WAL. So AC #2 is covered by opencode (CSP-603) plus this change.
Verification: `cache_invalidates_when_a_log_row_lands_only_in_the_wal` keeps a writer open in WAL mode with autocheckpoint off, asserts the main file's stamp is unchanged after the insert, and sees the new thread on the next cached call. With the main-file stamp it fails (probe run). Existing cache-hit and invalidation tests still pass.
<!-- SECTION:NOTES:END -->

## Final Summary

<!-- SECTION:FINAL_SUMMARY:BEGIN -->
The codex log query cache now keys on `SqliteStamp` (the logs database plus its `-wal`), so log rows committed to the WAL before a checkpoint are no longer hidden from a warm TUI or daemon. A survey found no other SQLite-backed cache keyed on the main file alone. Verified by a WAL test that fails with the old key.
<!-- SECTION:FINAL_SUMMARY:END -->
