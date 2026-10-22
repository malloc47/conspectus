---
id: CSP-673
title: Rebuild sessions by resuming them into a mux
status: To Do
assignee: []
created_date: '2026-10-22 01:11'
labels:
  - history-pins
milestone: m-23
dependencies:
  - CSP-672
documentation:
  - docs/plans/history-rebuild-and-pin-slots.md
priority: medium
type: feature
ordinal: 731000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
A session slot is a bookmark on a conversation; rebuilding it means resuming it. The natural place is the mux it last ran in (rebuilt if it's a ghost), or a new mux when that mux is live and busy. This covers CSP-170 (resume an un-muxed session into a mux); close that task when this lands.

- Scope: a session executor that resolves the lineage head, picks the last-hosting mux (rebuilding it as a prerequisite when gone) or a new mux named after it, and resumes the session there; History entries for a mux expand to their hosted sessions with "rebuild with this session".
- Tests: executor tests for a ghost last mux, a busy live last mux, and a session with no mux history.
- Manual checks: resume an old session from a still-live mux's history into a new mux.
- Blockers: the rebuild engine ticket.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 Rebuilding a session resumes its lineage head in its last mux, rebuilt if needed, or in a new mux when that one is busy
- [ ] #2 Any hosted session in a mux's history can be rebuilt
- [ ] #3 CSP-170 is closed as covered
<!-- AC:END -->

## Definition of Done
<!-- DOD:BEGIN -->
- [ ] #1 `just check` passes, or `git diff --check` for docs-only changes
- [ ] #2 Docs and ADRs are updated when behavior, architecture, or workflow change
<!-- DOD:END -->
