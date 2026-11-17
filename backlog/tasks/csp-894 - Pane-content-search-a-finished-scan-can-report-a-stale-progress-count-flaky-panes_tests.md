---
id: CSP-894
title: >-
  Pane content search: a finished scan can report a stale progress count (flaky
  panes_tests)
status: Done
assignee:
  - '@claude'
created_date: '2026-11-17 08:58'
updated_date: '2026-11-17 13:55'
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

`an_unreadable_pane_counts_as_skipped` asserts `found.progress.scanned == 3` through the same helper, so it has the same exposure; the other progress assertions in that file read `total` or `skipped`, which no race moves.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [x] #1 A pane scan's last progress event always reports every pane scanned (or the cancelled count), whatever order its workers finish in: it ends with an authoritative report after the workers join, as the session scan does, and unified search never lets a source's reported progress go backwards
- [x] #2 A test makes the interleaving certain rather than rare (for example workers released in a fixed order against a fake whose captures block), and it fails against the current code; the pane tests that assert progress no longer depend on timing
- [x] #3 The gate's flake list drops this test, and the fix is described in the commit that makes it
<!-- AC:END -->

## Definition of Done
<!-- DOD:BEGIN -->
- [x] #1 `just check` passes, or `git diff --check` for docs-only changes
- [x] #2 Docs and ADRs are updated when behavior, architecture, or workflow change
<!-- DOD:END -->

## Final Summary

<!-- SECTION:FINAL_SUMMARY:BEGIN -->
The pane scan's workers counted a pane scanned and then reported the count, so a worker held up between the two could be heard after the last and leave a finished scan on a smaller count; that was the flaky `matching_panes_report_line_snippet_and_highlight` and, in the product, a Search view left at "1 of 3". `PaneSource::run` now ends with an authoritative progress report after its workers join (as the session scan does), and the unified search keeps each source's count from going backwards, so `Done` and the live sum cannot read a stale one. A new test holds the first worker's report until another has reported every pane (two workers, a bounded wait), and fails against the old code with `scanned: 1` last (checked by removing the fix); another feeds the unified search an earlier count after a later one. The gate's flake list (my notes, not the repository's) drops the test. Gate: `just check`, `cargo test`, the separation rehearsal, `just plugins`, nextest with no `conspectus` on PATH, and `backlog doctor` all pass on the first run.
<!-- SECTION:FINAL_SUMMARY:END -->
