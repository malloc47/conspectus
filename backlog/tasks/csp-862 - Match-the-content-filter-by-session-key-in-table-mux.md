---
id: CSP-862
title: Match the content filter by session key in table mux
status: Done
assignee:
  - '@claude'
created_date: '2026-11-13 02:51'
updated_date: '2026-11-14 00:57'
labels:
  - live-state
  - ls-cli
milestone: m-26
dependencies: []
priority: low
type: bug
ordinal: 947000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
- Scope: `output/mux.rs` builds `SessionMatchInputs` with `session_key: &agent.session_key.1`, which is the state scope of the `(harness, state_scope, session_key)` triple, so the content filter (`conspectus table mux --contains TEXT`) looks hits up by the state scope and drops every mux. Pass the session key (`.2`), as the other tables do. Found while adding the live-state filter (CSP-826).
- Tests: `table mux --contains` over a world whose session transcript matches keeps the mux.
- Blockers: none.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [x] #1 `conspectus table mux --contains TEXT` keeps a mux whose attached session's transcript mentions TEXT
- [x] #2 It drops the mux when no attached session's transcript matches
<!-- AC:END -->

## Definition of Done
<!-- DOD:BEGIN -->
- [x] #1 `just check` passes, or `git diff --check` for docs-only changes
- [x] #2 Docs and ADRs are updated when behavior, architecture, or workflow change
<!-- DOD:END -->

## Implementation Plan

<!-- SECTION:PLAN:BEGIN -->
1. Failing test: tests/cli_contains.rs runs `table mux --contains` through the CLI over a fake tmux listing one session whose attached Claude Code transcript matches; the mux must stay for a matching query and leave for a non-matching one.
2. Fix output/mux.rs: pass the session key (`.2` of the triple), not the state scope, to SessionMatchInputs.
3. CHANGELOG entry; narrow tests, checks.sh, just-check.sh; commit.
<!-- SECTION:PLAN:END -->

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
Validation: the new CLI test in tests/cli_contains.rs failed before the fix (the --contains table listed no mux) and passes after; checks.sh clean; just check passed (nextest 5692 passed, 1 skipped).
<!-- SECTION:NOTES:END -->

## Final Summary

<!-- SECTION:FINAL_SUMMARY:BEGIN -->
Fixed `conspectus table mux --contains TEXT`: `output/mux.rs` built each attached agent's `SessionMatchInputs` with the state scope (`.1` of the `(harness, state scope, session key)` triple) as the session key, so the content filter found no hit and the table listed no mux. It now passes the session key (`.2`), as the other tables do. CHANGELOG has a Fixed entry; no other doc described the broken behavior.

Verification: a new CLI test (`contains_keeps_a_mux_whose_session_s_transcript_mentions_the_text`) runs the binary over a fake tmux listing one session that hosts a Claude Code transcript; it failed before the fix and passes after (the mux stays for `QUOKKA`, leaves for `zebra`). `checks.sh` (fmt, clippy, rustdoc, diff-check) clean; `just check` passed (5692 tests, 1 skipped).

**Regressions:** none. **Consistency wins:** `table mux --contains` now agrees with `table sessions --contains` and the TUI's Mux view.
<!-- SECTION:FINAL_SUMMARY:END -->
