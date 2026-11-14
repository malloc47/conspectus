---
id: CSP-835
title: Decide whether --no-resume-view should stop writing tui-state.json
status: Done
assignee:
  - '@claude'
created_date: '2026-11-12 01:58'
updated_date: '2026-11-14 15:04'
labels:
  - tui
milestone: m-23
dependencies: []
priority: low
type: task
ordinal: 892000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
`conspectus tui --no-resume-view` skips reopening the persisted view (and, since CSP-702, each view's expanded and selected rows), but the run still enables persistence: `event_loop` and `static_event_loop` in `src/tui/runtime.rs` call `resumed_app`, which sets the `TuiStateCache`, so view switches, settings changes, and the end of the run write `$XDG_STATE_HOME/conspectus/tui-state.json` as usual, including `last_view`. The help text used to say the flag "does not write the sidecar on view switches", and comments in `src/tui/app.rs` and `src/tui/runtime.rs` said the cache was left unset under the flag; CSP-702 corrected the help and the comments to match the code, and `docs/operations.md` ("TUI state", Opt-outs) now says the run's own changes are still written.

Decide which is wanted. Option A (today): the flag only skips what is reopened; a scripted run's last view and rows become the next interactive run's. Option B: the flag also skips every write (no `enable_view_persistence`), which then also skips restoring sort, filters, and groupings, unless restoring is split from writing (a read-only cache mode on `App`).

Scope: the decision, the code for option B if chosen, `--help`, `docs/operations.md`, and a CLI test in `tests/cli_tui_state_invariants.rs` that pins the chosen behavior (a `--no-resume-view` run cannot be driven headless today; `tui --fixture` with scripted keys or a unit test through `resumed_app` may do). Blockers: none.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [x] #1 Decision (operator, 2026-11-13): `conspectus tui --no-resume-view` skips every write to `tui-state.json` as well as reopening the persisted view and rows, and still applies the saved sort, filters, and groupings; an amendment to ADR 0202 records it
- [x] #2 A `--no-resume-view` run leaves the state file byte-identical, and creates none when it is absent, however the run switches views, changes settings, hands off the terminal, or ends; a test pins it
- [x] #3 The same run still applies the saved sort, filters, and groupings (CLI flags still win), and a run without the flag writes as before; a test pins both
- [x] #4 `--help`, `docs/operations.md` (TUI state, startup precedence), `CHANGELOG.md`, and the code comments say what the flag does, with no ADR or backlog reference in user-facing text
<!-- AC:END -->

## Definition of Done
<!-- DOD:BEGIN -->
- [x] #1 `just check` passes, or `git diff --check` for docs-only changes
- [x] #2 Docs and ADRs are updated when behavior, architecture, or workflow change
<!-- DOD:END -->

## Implementation Plan

<!-- SECTION:PLAN:BEGIN -->
1. Tests first: unit tests through resumed_app (restore_tests.rs) for a no-resume run: file absent stays absent, file seeded stays byte-identical across a view switch, a settings change, the Exec hand-off write, and the end-of-run write; saved sort/filter/grouping still applied; a resume run still writes.
2. App: split restoring from writing (a read-only cache mode: the cache is read for restore, persist_state writes only when writes are enabled); resumed_app picks the mode from RunConfig::resume_view.
3. Update --help, docs/operations.md, CHANGELOG.md, comments, and the ADR 0202 amendment (decision 7 and the consequence that said the flag still writes).
4. checks.sh and just-check.sh, commit.
<!-- SECTION:PLAN:END -->

## Final Summary

<!-- SECTION:FINAL_SUMMARY:BEGIN -->
Operator decision of 2026-11-13: `conspectus tui --no-resume-view` now makes no write to `tui-state.json` (no view switch, settings change, hand-off, or end-of-run write, and no file or directory when it is absent), while still restoring the saved sort, filters, groupings, mux recency, and preview settings (command-line flags still win). It still reopens neither the last view nor any view's rows.

How: `App`'s state-file use is one of three modes (`StateFile::Off`, `ReadOnly`, `ReadWrite`) in `src/tui/app.rs`, replacing the single `tui_state_cache` option; `persist_state`, the only write path, writes in `ReadWrite` only, and `restore_persisted_state` reads in `ReadOnly` and `ReadWrite`. `resumed_app` (`src/tui/runtime.rs`) picks the mode from `RunConfig::resume_view`. The reducer still emits `Effect::Persist`; the write is what is skipped. `--snapshot` is unchanged (off).

Recorded as an amendment to ADR 0202 (status line, decision 7 and the stale consequence point at it; README index row updated), plus `docs/design.md`. `--help`, `docs/operations.md` (view precedence, TUI state: when it is written, Opt-outs, Read-only invariant), `CHANGELOG.md`, and the code comments say the same; no ADR or backlog reference in user-facing text.

Tests: unit tests in `src/tui/app/restore_tests.rs` (file byte-identical across a settings change, view switches, and the end-of-run write; no file or directory when absent; saved sort, filter, grouping, and wrap still applied; a resuming run still writes) and two tests of the real TUI in a private tmux server in `tests/tui_tmux_e2e.rs` (the saved file is byte-identical after a switch and quit; no file is created, while the same run without the flag creates one). The e2e tests fail when `resumed_app` is made to always write (checked). `tests/cli_tui_state_invariants.rs` cannot run the TUI headless; its module doc points at the two test sites.

Verified: `/tmp/csp-fu/checks.sh` clean; `just check` exit 0 (5752 passed, 1 skipped); the first `just check` hit the known `cli_history` tmux flake (`a_hand_made_mux_is_followed_through_a_rename_and_goes_gone_when_killed`), which passed alone and in the rerun. No visible TUI change besides the flag's behavior, so no frames compared.
<!-- SECTION:FINAL_SUMMARY:END -->
