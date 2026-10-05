---
id: CSP-552
title: Decide the backlog's long-term shape
status: Done
assignee: []
created_date: '2026-09-30 18:30'
updated_date: '2026-10-05 04:02'
labels:
  - rel
milestone: m-20
dependencies: []
priority: low
ordinal: 623000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
- Scope: this file is ~14k lines and `CSP-538` shows checkbox drift, so
  ADR 0009's trigger ("evaluate Backlog.md when the manual backlog
  becomes difficult to maintain") may have fired. Options: archive
  completed phases to a separate file, or migrate to Backlog.md. Decide
  via ADR.
- Blockers: none.
<!-- SECTION:DESCRIPTION:END -->

Legacy ID: `REL-021`

## Final Summary

<!-- SECTION:FINAL_SUMMARY:BEGIN -->
Decided by ADR 0109, accepted 2026-10-02, which supersedes ADR 0009: the backlog moved from `docs/backlog.md` to Backlog.md task files under `backlog/`, with every story converted and renumbered to `CSP-NNN` and `docs/backlog-legacy-ids.md` mapping the old IDs. Verified: ADR 0109 reads Accepted, `docs/backlog.md` is now a pointer to `backlog/`, and `backlog doctor` reports no problems.
<!-- SECTION:FINAL_SUMMARY:END -->
