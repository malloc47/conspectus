---
id: CSP-891
title: 'Pi: a compaction with nothing to compact is not an error'
status: Done
assignee:
  - '@claude'
created_date: '2026-11-16 22:47'
updated_date: '2026-11-17 01:01'
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
- [x] #1 A `session_compact_failed` whose error message says there was nothing to compact leaves the state as it was before the compaction began (no `error`), with the text read for the match and never kept or sent
- [x] #2 A compaction that fails for any other reason, and an overflow compaction that fails while a run is active, still reads `error/failed` as before
- [x] #3 The extension's tests and the Pi fixtures README cover both, and ADR 0206's decision 25 or an ADR amendment records the read of the message
<!-- AC:END -->

## Definition of Done
<!-- DOD:BEGIN -->
- [x] #1 `just check` passes, or `git diff --check` for docs-only changes
- [x] #2 Docs and ADRs are updated when behavior, architecture, or workflow change
<!-- DOD:END -->

## Final Summary

<!-- SECTION:FINAL_SUMMARY:BEGIN -->
The Pi extension no longer reads a manual `/compact` of a session too small to compact as a failure. It reads the failed compaction's `errorMessage` for "nothing to compact" (case-insensitive, the first 4096 characters, dropped after the read like the classifier's), and the state returns to what it was; any other failure, and an overflow failure during a run, still read `error/failed`. The extension is version 4, so `hook status pi` reports older copies as outdated (the version also carries CSP-822's quit and closed-pane fix), and the CLI smoke test now reads the declared version from the source instead of spelling it. Verified with unit tests for the matcher, a new replay script (`compaction-nothing-to-compact.jsonl`, also replayed by the Rust adapter tests) that fails against the old code, and a real Pi 1.5.2 session whose state stayed `ready/finished` through the same `/compact`. ADR 0206 (decision 25) and ADR 0243 record the read; the fixtures README, the research note, and the changelog carry it. Gate on the final commit: fmt, clippy, rustdoc, `git diff --check`, `cargo test`, the separation rehearsal, `just plugins`, and nextest with no `conspectus` on PATH all pass; `just check`'s own nextest run hit the known tmux start-up flake (CSP-886, `no_resume_view_creates_no_state_file_but_a_run_without_it_does`, which passes 5 of 5 alone and in the other two full runs).
<!-- SECTION:FINAL_SUMMARY:END -->
