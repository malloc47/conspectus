---
id: CSP-820
title: Stop Pi idle cache warming from making a session look recently active
status: To Do
assignee: []
created_date: '2026-11-12 01:47'
labels:
  - live-state
  - ls-fix
milestone: m-26
dependencies: []
documentation:
  - docs/plans/live-state/research/04-pi.md
priority: low
type: bug
ordinal: 906000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Pi's default idle cache warming appends `usage` entries to the session file for up to 30 minutes after a run. The adapter derives `last_active_epoch` from the file's mtime, so an idle Pi session can look active, which will also mislead any live-state fallback and the age column.

Scope: In the Pi adapter, take `last_active_epoch` from the last non-`usage` entry's timestamp instead of the file mtime, keeping the mtime as a fallback for files it cannot parse.

TUI changes: Ages for Pi sessions that were warming a cache become older, which is correct. List any changed age in the final summary.

Tests and checks: A fixture with an assistant stop followed by cache-warm `usage` entries; the existing Pi adapter tests.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 A Pi session whose last entries are cache-warm `usage` records reports the earlier activity time
<!-- AC:END -->

## Definition of Done
<!-- DOD:BEGIN -->
- [ ] #1 `just check` passes, or `git diff --check` for docs-only changes
- [ ] #2 Docs and ADRs are updated when behavior, architecture, or workflow change
<!-- DOD:END -->
