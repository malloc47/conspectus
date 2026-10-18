---
id: CSP-634
title: Stamp the codex log query cache with the database's WAL file
status: To Do
assignee: []
created_date: '2026-10-18 06:41'
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
- [ ] #1 A codex log row committed only to the WAL is visible to the next cached query
- [ ] #2 Every SQLite-backed discovery cache keys on the database and its WAL
<!-- AC:END -->

## Definition of Done
<!-- DOD:BEGIN -->
- [ ] #1 `just check` passes, or `git diff --check` for docs-only changes
- [ ] #2 Docs and ADRs are updated when behavior, architecture, or workflow change
<!-- DOD:END -->
