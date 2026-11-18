---
id: CSP-896
title: 'Showcase: the interactive demo shows live state and unread marks'
status: In Progress
assignee:
  - '@claude'
created_date: '2026-11-17 23:50'
updated_date: '2026-11-17 23:51'
labels:
  - tui
dependencies: []
ordinal: 972000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
just demo (tui --fixture) and the scenario TUI read the showcase's live states and ages against today's clock, so every agent reads stale and no unread mark shows; and their read marks are written to the operator's real state file, so a mark made in one demo run hides the unread marks of the next. The demo should show what the screenshots show, and be repeatable.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 tui --fixture accepts the showcase's anchor time (--snapshot-now) and just demo passes it, so its live marks, unread marks, and ages match the screenshots
- [ ] #2 Marking read in a fixture or scenario TUI (c, C, resting the cursor) is kept in memory: the operator's state file gains no read marks from them and its existing marks do not hide the demo's
- [ ] #3 Editing the fixture so a working agent waits, then pressing r, shows the new stop unread (a documented, tested recipe)
- [ ] #4 README, docs/dev-scenarios.md, and the justfile say how to see unread in the demo
<!-- AC:END -->

## Definition of Done
<!-- DOD:BEGIN -->
- [ ] #1 `just check` passes, or `git diff --check` for docs-only changes
- [ ] #2 Docs and ADRs are updated when behavior, architecture, or workflow change
<!-- DOD:END -->
