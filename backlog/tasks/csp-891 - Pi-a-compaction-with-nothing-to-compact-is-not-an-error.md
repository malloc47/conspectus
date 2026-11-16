---
id: CSP-891
title: 'Pi: a compaction with nothing to compact is not an error'
status: In Progress
assignee:
  - '@claude'
created_date: '2026-11-16 22:47'
labels:
  - live-state
  - ls-emit
milestone: m-26
dependencies:
  - CSP-822
priority: low
type: bug
ordinal: 967000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
CSP-822 recorded a real Pi 1.5.2 session: a manual `/compact` of a session too small to compact fires `session_compact_failed` with `errorMessage` "Nothing to compact (session too small)", and the extension reads it as a failed compaction, so the session shows `error/failed` until its next run or event, although nothing went wrong. `tests/fixtures/hooks/recorded/pi/tool-abort-compact-failed-new-reload.jsonl` holds the sequence, and `compaction-failed.jsonl` holds the synthetic one that is right for a real failure.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 A `session_compact_failed` whose error message says there was nothing to compact leaves the state as it was before the compaction began (no `error`), with the text read for the match and never kept or sent
- [ ] #2 A compaction that fails for any other reason, and an overflow compaction that fails while a run is active, still reads `error/failed` as before
- [ ] #3 The extension's tests and the Pi fixtures README cover both, and ADR 0206's decision 25 or an ADR amendment records the read of the message
<!-- AC:END -->

## Definition of Done
<!-- DOD:BEGIN -->
- [ ] #1 `just check` passes, or `git diff --check` for docs-only changes
- [ ] #2 Docs and ADRs are updated when behavior, architecture, or workflow change
<!-- DOD:END -->
