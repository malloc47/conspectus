---
id: CSP-894
title: >-
  Pane content search: a finished scan can report a stale progress count (flaky
  panes_tests)
status: To Do
assignee: []
created_date: '2026-11-17 08:58'
labels:
  - test
  - content-search
dependencies: []
priority: medium
type: bug
ordinal: 970000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
`content_search::panes_tests::matching_panes_report_line_snippet_and_highlight` failed once in the pre-main gate of 2026-11-16 under load (`left: Progress { scanned: 1, total: 3, skipped: 0 }`, `right: Progress { scanned: 3, total: 3, skipped: 0 }`) and passed 15 of 15 runs alone and in every other full run. It is not the test's alone.

Cause, read from `src/content_search/panes.rs` (`PaneSource::run`): each worker does `scanned.fetch_add(1)`, then builds `progress()` and calls `emit`. Between a worker's increment and its emit, another worker can increment and emit first, so the events reach the consumer out of order and the last one can carry a smaller `scanned` than an earlier one. The test's `run` helper keeps the last `Progress` it sees, so it reads a stale count, and so does the product: `unified.rs` stores the latest progress of each source (`latest[index] = progress`) and sums them, so a finished pane scan can leave the Search view reading `1 of 3 scanned`. The session scan (`scan.rs`) does not have this, because it ends with an authoritative `finish(progress(), ...)` after its workers join; the pane source ends with no final report.

The pane source has two other tests that read `found.progress` (the neighbors of the failing one), and the same race can fail any of them under load.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 A pane scan's last progress event always reports every pane scanned (or the cancelled count), whatever order its workers finish in: it ends with an authoritative report after the workers join, as the session scan does, and unified search never lets a source's reported progress go backwards
- [ ] #2 A test makes the interleaving certain rather than rare (for example workers released in a fixed order against a fake whose captures block), and it fails against the current code; the pane tests that assert progress no longer depend on timing
- [ ] #3 The gate's flake list drops this test, and the fix is described in the commit that makes it
<!-- AC:END -->

## Definition of Done
<!-- DOD:BEGIN -->
- [ ] #1 `just check` passes, or `git diff --check` for docs-only changes
- [ ] #2 Docs and ADRs are updated when behavior, architecture, or workflow change
<!-- DOD:END -->
