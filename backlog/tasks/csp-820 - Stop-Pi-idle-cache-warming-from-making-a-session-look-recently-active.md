---
id: CSP-820
title: Stop Pi idle cache warming from making a session look recently active
status: Done
assignee:
  - '@claude'
created_date: '2026-11-12 01:47'
updated_date: '2026-11-12 13:30'
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
- [x] #1 A Pi session whose last entries are cache-warm `usage` records reports the earlier activity time
<!-- AC:END -->

## Definition of Done
<!-- DOD:BEGIN -->
- [x] #1 `just check` passes, or `git diff --check` for docs-only changes
- [x] #2 Docs and ADRs are updated when behavior, architecture, or workflow change
<!-- DOD:END -->

## Implementation Plan

<!-- SECTION:PLAN:BEGIN -->
1. In `src/discovery/harness/pi.rs` `read_tail`, skip `usage` entries (cache warming writes them while the session is idle) when picking the newest entry timestamp; the preview reader is unchanged.
2. Keep the fallback chain for a tail with no usable entry: the header timestamp, then the file mtime.
3. Tests in `pi_tests.rs`: an assistant stop followed by cache-warm `usage` entries reports the stop's time; a later real entry still moves the time; the existing newest-entry test stays.
4. Check goldens and showcase frames (frames.sh before and after); list any changed age.
5. Docs: module comment, ADR 0121 tier note if it names the timestamp source, plan README section 8, CHANGELOG Fixed entry.
<!-- SECTION:PLAN:END -->

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
`read_tail` in `src/discovery/harness/pi.rs` now ignores `usage` entries when picking the newest timestamp; the fallback chain (header timestamp, then mtime) is unchanged. A tail that holds only `usage` entries falls back to the header timestamp, which errs older; reaching that needs more than 32 KiB of warming entries after the last turn, which half an hour of warming does not write.
Validation: `run-some.sh --lib discovery::harness::pi` (66 passed), `run-some.sh --test fixture_corpus pi_` (5 passed), `checks.sh` exit 0, `run-tests.sh` exit 0 (5371 passed, 0 failed, 1 ignored). frames.sh before and after: all nine showcase frames byte-identical (the showcase Pi session has no `usage` entries, and the frames read the showcase graph fixture).
<!-- SECTION:NOTES:END -->

## Final Summary

<!-- SECTION:FINAL_SUMMARY:BEGIN -->
Pi sessions now take `last_active_epoch` from the newest tail entry that is not `usage`, so idle cache warming (which appends `usage` entries for up to 30 minutes after a run) no longer makes a session look recently active. Files with no usable entry still fall back to the header timestamp, then the mtime.

Tests: a synthetic fixture `tests/fixtures/pi/session-cache-warm.jsonl` (assistant stop at 12:01, cache-warm entries to 12:14) asserts the stop's time in `fixture_corpus`; unit tests cover a later real entry moving the time again and the mtime fallback.

Changed ages: none in any golden or showcase frame (no existing fixture had a `usage` entry; all nine frames.sh frames are byte-identical). For an operator's real idle Pi sessions, the age now counts from the last turn instead of the last cache warm.

Verified with `checks.sh` (fmt, clippy, rustdoc, diff check) and the full `run-tests.sh` (5371 passed, 0 failed). CHANGELOG Fixed entry and the plan README bug list updated.
<!-- SECTION:FINAL_SUMMARY:END -->
