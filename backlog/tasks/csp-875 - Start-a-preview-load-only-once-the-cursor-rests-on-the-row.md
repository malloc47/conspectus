---
id: CSP-875
title: Start a preview load only once the cursor rests on the row
status: Done
assignee:
  - '@claude'
created_date: '2026-11-13 20:40'
updated_date: '2026-11-14 22:16'
labels:
  - h-scroll
  - h-preview
  - tui
milestone: m-11
dependencies: []
references:
  - src/tui/runtime.rs
  - src/tui/preview/mod.rs
  - docs/adr/0112-node-specific-previews.md
  - docs/adr/0024-tui-runtime-and-architecture.md
priority: medium
type: bug
ordinal: 899000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Every cursor move starts a preview load for the row it lands on, even when the cursor is only passing through. `plan_preview_load` (`src/tui/runtime.rs`) loads again whenever the selection's source differs from the one last shown, cached or not.

- A repo row runs four git commands: `symbolic-ref`, `remote -v`, `rev-parse`, and `log -n 30`.
- A checkout row runs `git status`.
- A session row reads its transcript.

Each runs on a worker thread that keeps going after the cursor moves on. A live mux row instead runs `tmux capture-pane` on the UI thread; moving that off the UI thread is its own task.

Measured 2026-11-13 with strace on the release build: a 90-key down/up sweep started 134–198 git processes in the sessions and mux views, about two per key, all for rows the operator only scrolled past. Those processes compete for CPU with the UI thread just when it is working through a scroll backlog (CSP-874), and the stall is worst on a loaded machine.

The content search already waits for its query to rest before it scans (`search_overlay_tick`).
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [x] #1 Moving the cursor through rows without stopping starts no preview load for the rows it passes, and the row it comes to rest on starts its load within 150 ms of the last key
- [x] #2 A row passed during a move shows its cached preview if it has one and the loading placeholder otherwise, as today
- [x] #3 A load still running when the cursor leaves its row starts no further subprocesses for that row
- [x] #4 `r`, a hard refresh, the mux preview interval refresh, and the reuse of loaded pull-request previews behave as they do today
- [x] #5 Tests cover a burst of moves that starts one load, for the row the cursor rests on, and a single move followed by a pause that loads the new row
- [x] #6 Repeating the 90-key sweep from the description starts git only for the rows the cursor rested on, and the final summary records the before and after process counts
<!-- AC:END -->

## Definition of Done
<!-- DOD:BEGIN -->
- [x] #1 `just check` passes, or `git diff --check` for docs-only changes
- [x] #2 Docs and ADRs are updated when behavior, architecture, or workflow change
<!-- DOD:END -->

## Implementation Plan

<!-- SECTION:PLAN:BEGIN -->
1. Confirmed against HEAD: `plan_preview_load` loads whenever the selection's source differs from the one last shown, cached or not, and `LiveMode::load_preview` starts a worker per load that nothing stops; the git loaders run four commands in a row.
2. Pacing: `PreviewRest` (new `src/tui/runtime/previews.rs`) tracks the source the cursor is on and when input moved it there; the loop asks it each pass for the source to load, and it says none until the cursor has rested 100 ms (a source change with no input behind it, such as the first snapshot, loads at once; `r` and a hard refresh load the selection at once). The poll timeout shortens to the rest deadline so the load starts within 150 ms of the last key.
3. Cancellation: `PreviewLoads` owns the workers and a stop flag per load; the loop tells it where the cursor is each pass and it stops every other running load; the git loaders ask a stop predicate before each command and end `Stopped`; a stopped load sends `PreviewLoadAbandoned`, which ends its loading mark and marks the cached entry stale so it reloads when the cursor rests on it again.
4. Tests first: a burst of moves loads once (the row it rests on, within 150 ms), a single move then a pause loads the new row, a row passed shows its cached preview or the placeholder, `r` loads at once, reuse of loaded PR previews, a git load stopped between commands starts no more, an abandoned load reloads.
5. Count git processes under strace for a 90-key sweep before and after on a copy of the real data (numbers only); frames.sh byte-identical; ADR 0237; operations.md and changelog; `just check`; commit.
<!-- SECTION:PLAN:END -->

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
Verification: loop tests in `src/tui/runtime/loop_tests.rs` run the real loop over a scripted console with a virtual clock and a Probe mode that stamps and answers each preview load: a 20-key burst loads once (row 020 at 100 ms after the last key); 30 keys 20 ms apart load once, within the rest and 150 ms; one move then a pause loads the new row at 100 ms; a row passed shows the placeholder, and a row passed and returned to before the rest shows its cached preview with no new load; `r` loads at once. With the rest set to zero, four of these fail. `src/tui/runtime/previews_tests.rs`: a load for another row is stopped and abandoned (cached entry marked stale), a load for the cursor's row keeps running, a loader panic still ends the load. `src/tui/preview/git_tests.rs`: for status, repo, ref with ahead/behind, and show, stopping after every number of commands ends the load with exactly that many started and the refused one never run. Existing `preview_loading` tests (PR reuse until the forge interval, hard refresh, failed PR retry, reselect reloads transcript, no-live-preview) pass unchanged. The mux preview interval refresh named in the story does not exist yet (`mux_preview_interval` is parsed and unused, CSP-185), so there was nothing to preserve beyond the stale-mark reload after a hand-off, which plan_preview_load still decides.
Measurement (release build, sessions view, 184x85, operator's real data on a COPY of the state and cache dirs, tmux and forge off, strace -f execve counting git commands that ran, startup git work waited out first so a 0-key control shows 0; numbers only; harness /tmp/csp-fu/wheel/pty_sweep.py, not committed): 90-key sweep in 15-row down/up sweeps, 25 keys/s (a held key): 174 git commands before, 0 after (the sweep returns to its row). 200 keys/s: 60-72 before (pre-batching 60-72, post-batching 56-60), 0 after. 80 keys at 200 keys/s, ending 5 rows below the start on a repo row: 56-72 before, 4 after (one repo row's symbolic-ref, rev-parse, log, remote). frames.sh: nine frames byte-identical to the pre-CSP-874 baseline.

Method (harness not committed, as for CSP-874): the TUI runs under `strace -f -qq -e trace=execve` in a pty on copies of the state and cache directories with `--refresh-interval 1h`; the harness waits until the strace log has been still for 3 s (startup discovery runs git across every repo), types 15-row down/up sweeps of cursor keys at the given rate, waits for output to go quiet, and counts the `git` execve lines that ran (`= 0`; strace also logs each failed PATH lookup) added since the keys began. A run with no keys counts 0. Only counts and subcommand names are printed.
<!-- SECTION:NOTES:END -->

## Final Summary

<!-- SECTION:FINAL_SUMMARY:BEGIN -->
A preview load now starts once the cursor has rested on its row for 100 ms. `PreviewRest` (`src/tui/runtime/previews.rs`) tracks the source the Preview shows and when input last moved it; each pass of the loop asks it which source to load, and the loop's wait for input shortens to the rest deadline so the row the cursor stops on loads about 100 ms after the last key. A source that changes with no key behind it (the first snapshot) loads at once, `r` and a hard refresh load the selection at once, and what shows meanwhile is unchanged (cached preview or the loading placeholder). The preview workers moved into `PreviewLoads`, which stops every load running for a source other than the cursor's: the git loaders ask a stop predicate before each command (`load_git_preview_unless`), a stopped load stores nothing and sends the new `Msg::PreviewLoadAbandoned`, whose reducer arm ends the loading mark and marks any cached entry stale so it reloads when the cursor rests on it again. ADR 0237 (amends 0112 section 5), operations.md, and the changelog record it.

Verified: loop tests (burst loads once for the row it rests on, within 150 ms; single move then pause loads the new row; passed rows show cached preview or placeholder; `r` immediate), worker tests (stop, abandon, panic), git tests (no command starts after a stop, at every stop point), the unchanged `plan_preview_load` tests (PR reuse, hard refresh); frames.sh byte-identical; `just check` passes (5770 tests).

Before and after on the operator's data (release build, sessions view, numbers only): a 90-key sweep at held-key speed (25 keys/s) started 174 git commands before and 0 after; at 200 keys/s, 56-72 before and 0 after; a sweep ending on another repo row started 4 after (that row's four). Not verified: the mux view on real data (needs the host tmux server); the mux preview interval refresh does not exist yet (CSP-185).

Visible changes: a row the cursor stops on shows its fresh preview about 100 ms after the last key instead of at once; rows passed during a move never load. Regressions: none known. Consistency wins: none.
<!-- SECTION:FINAL_SUMMARY:END -->
