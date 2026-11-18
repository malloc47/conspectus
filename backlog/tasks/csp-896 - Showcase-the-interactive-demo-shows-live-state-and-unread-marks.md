---
id: CSP-896
title: 'Showcase: the interactive demo shows live state and unread marks'
status: Done
assignee:
  - '@claude'
created_date: '2026-11-17 23:50'
updated_date: '2026-11-18 00:40'
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
- [x] #1 tui --fixture accepts the showcase's anchor time (--snapshot-now) and just demo passes it, so its live marks, unread marks, and ages match the screenshots
- [x] #2 Marking read in a fixture or scenario TUI (c, C, resting the cursor) is kept in memory: the operator's state file gains no read marks from them and its existing marks do not hide the demo's
- [x] #3 Editing the fixture so a working agent waits, then pressing r, shows the new stop unread (a documented, tested recipe)
- [x] #4 README, docs/dev-scenarios.md, and the justfile say how to see unread in the demo
<!-- AC:END -->

## Definition of Done
<!-- DOD:BEGIN -->
- [x] #1 `just check` passes, or `git diff --check` for docs-only changes
- [x] #2 Docs and ADRs are updated when behavior, architecture, or workflow change
<!-- DOD:END -->

## Final Summary

<!-- SECTION:FINAL_SUMMARY:BEGIN -->
`tui --fixture` takes `--snapshot-now` and `just demo` passes the showcase's anchor time, so the interactive demo shows the live states and unread rows of the screenshots. Fixture and scenario TUIs keep read marks in memory (`App::keep_read_marks_in_memory`), so the operator's state file gains none and its existing marks do not hide the demo's. `just demo` runs on a copy under target/demo and `just demo-stop` stages a stop in it for `r` to load (ADR 0248). Verified: a unit test of the memory-only marks; `tests/tui_fixture_demo.rs` drives the real TUI in a private tmux server (live states show, a seeded state-file mark is ignored, `c` reads, a staged stop shows after `r`, nothing is written back, the next run starts unread, and without the anchor time everything reads stale); `just demo-stop` run on a copy changed only the hook session's entry; the full pre-main gate passed.
<!-- SECTION:FINAL_SUMMARY:END -->
