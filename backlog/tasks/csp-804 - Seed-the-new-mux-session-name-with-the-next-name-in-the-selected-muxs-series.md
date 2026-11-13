---
id: CSP-804
title: Seed the new mux session name with the next name in the selected mux's series
status: Done
assignee:
  - '@claude'
created_date: '2026-11-11 22:33'
updated_date: '2026-11-11 23:10'
labels:
  - tui
dependencies: []
priority: medium
type: bug
ordinal: 890000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Why: a common workflow is starting `worker-2` next to `worker-1`. Pin creation once pre-filled the next-in-series name (bump the trailing number of the selected row's name until it is free; `-2`, `-3`, ... when it has none), across live mux names and pin ids. Pin slots replaced the pin create form, and nothing carried the seeding over: today `derive_new_mux_name` in `src/tui/runtime.rs` returns an empty string, so the new-session form (`n`, or the mux menu) and the mux launch form both open with an empty name and the operator retypes it every time.

Scope: a pure series helper (`work-1` -> `work-2`, zero padding kept, `name` -> `name-2`, first candidate always the incremented one, digits that overflow treated as no number) with unit tests; seed both forms' name field from the mux the operator is looking at (the action subject: a mux row or node, live or gone; an agent session's preferred mux; a mux pin with no node); groups, PRs, nothing selected, and other rows seed empty. "Taken" is a live mux session name, a history mux record name (the rule the rebuild's mux naming already uses, shared rather than duplicated), or a pin id. Comments and docs that call the seed intentionally empty are corrected; `CHANGELOG.md` gets a Fixed entry.

TUI changes:
- Consistency wins: the new-session and mux launch forms open with a usable name for a selected mux, matching what pin creation used to do
- Regressions or behavior changes: a form opened from a mux row is no longer empty; an operator who wanted a different name clears the field first (`Ctrl-U`)

Tests and checks: reducer tests through `Msg::OpenNewMuxForm` and `Msg::OpenMuxLaunchForm` (numbered mux with the next taken, session row linked to a mux, pin id skipped, no digits, zero padding, group row or no selection empty, right-pane focus); helper unit tests; `conspectus tui --snapshot` of the form before and after.

Blockers: none.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [x] #1 With a mux `work-1` selected and live muxes `work-1` and `work-2`, both the new-session form and the mux launch form open with the name `work-3`
- [x] #2 An agent session row seeds from its mux; a mux pin with no node seeds from its id; with the right pane focused, the entity it shows is the base
- [x] #3 Names taken by a live mux, a history mux record, or a pin id are skipped; a name without trailing digits seeds `name-2`; zero padding keeps its width (`work-007` -> `work-008`, `work-099` -> `work-100`)
- [x] #4 A group row, a PR row, or no selection opens the forms with an empty name, as before
- [x] #5 Comments, `docs/operations.md`, and `CHANGELOG.md` describe the seeded name; the snapshot of the form shows the seed
<!-- AC:END -->

## Definition of Done
<!-- DOD:BEGIN -->
- [x] #1 `just check` passes, or `git diff --check` for docs-only changes
- [x] #2 Docs and ADRs are updated when behavior, architecture, or workflow change
<!-- DOD:END -->

## Implementation Plan

<!-- SECTION:PLAN:BEGIN -->
1. Share the taken-name rule: move the predicate in `unique_mux_name` (`src/rebuild/session.rs`) to a `pub(crate)` `GraphSnapshot::mux_name_taken` in `src/model/graph.rs` (a mux node, live or gone, or a history mux record with that session name) and call it from the rebuild.
2. Add `src/tui/runtime/mux_name.rs`: a pure `next_in_series(base, taken)` (trailing ASCII digits bumped with zero padding kept, overflow treated as no number, `-2`, `-3`, ... otherwise, the first candidate always the incremented one) with unit tests, and `seed(app)`: the base from `action_subject` (a mux node: live session name, else its history record's recipe name, else the native id without its `tmux:<socket>:` prefix; an agent session: the mux its row shows; a mux pin with no node: its effective `name`, which defaults to the pin id; anything else empty), taken = `mux_name_taken` or a pin id.
3. Tests first, in `src/tui/consistency_tests/forms.rs`, through `Msg::OpenNewMuxForm` and `Msg::OpenMuxLaunchForm`: numbered mux with the next taken, session row, pin id skipped, no digits, zero padding, group and PR rows and no selection empty, right-pane focus; watch them fail against the empty seed.
4. Wire `derive_new_mux_name` to the seed (both forms use it); fix the stale comments; fix any test that assumed an empty name.
5. Docs: `docs/operations.md` (both forms' name field), `CHANGELOG.md` Fixed entry, operator-review rows for the judgment calls.
6. Verify with `conspectus tui --snapshot` before and after; run fmt, clippy (workspace and tuikit), the isolated suite, rustdoc, examples, `git diff --check`; commit in slices; finalize.
<!-- SECTION:PLAN:END -->

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
- Tests first: `fae16368` adds `next_in_series` with its unit tests and the consistency tests in `src/tui/consistency_tests/forms.rs`; seven seeding tests failed there (`left: ["", ""]`), as the forms opened empty. `867359e9` makes them pass.
- The base comes from `action_subject`. Two refinements of the brief, recorded in `docs/plans/tui-framework/operator-review.md`: a session takes the mux its row shows (`GraphSnapshot::mux_host_of`, which falls back to the mux history says hosted it last), and a mux pin with no node takes its effective `name` field (the pin id unless overridden).
- The taken rule of `unique_mux_name` is now `GraphSnapshot::mux_name_taken` in `src/model/graph.rs`, called by the rebuild and the TUI, so the TUI does not import the rebuild module.
- The right-pane test needed a world where the pane's entity differs from the row's: a child session in `work-1` whose parent runs in `build-3` (the consistency world's right panes only link to the same mux).
- Checks: `cargo fmt --all -- --check`, `cargo clippy --workspace --all-targets --all-features -- -D warnings`, `cargo clippy -p tuikit --all-targets -- -D warnings`, the isolated suite (4864 passed, 0 failed, 1 ignored; baseline 4849 plus 15 new), `RUSTDOCFLAGS='-D warnings' cargo doc --workspace --no-deps`, `cargo build --examples`, `git diff --check`: all pass. `just check` itself was not run, because its `cargo test` is not tmux-isolated; nextest runs the same tests, and `tuikit` is unchanged.
<!-- SECTION:NOTES:END -->

## Final Summary

<!-- SECTION:FINAL_SUMMARY:BEGIN -->
Restored the name seed that pin creation had before pins replaced its form. The new tmux session form (`n`, the mux menu's bare-shell entry) and the launch form (`N`) now open with the next free name after the mux the operator is looking at: the action subject (the right pane's entity while it has focus, else the selected row), so a mux row (live or gone), the mux a session's row shows, or a mux pin with no node. The trailing number is bumped with its zero padding kept as a minimum width (`work-007` -> `work-008`, `work-099` -> `work-100`); a name without one gets `-2`, `-3`, and so on; the first candidate is always the next one. A name is taken when a mux node (live or gone), a history mux record, or a pin id has it. The first two make up the rebuild's rule, now one shared `GraphSnapshot::mux_name_taken`. A group, PR, checkout pin, session outside any mux, or no selection still opens the forms empty.

Commits: `fae16368` (helper, unit tests, and the seeding tests, which fail there), `867359e9` (the fix), and the docs and backlog commit. Code: `src/tui/runtime/mux_name.rs` (new), `src/tui/runtime.rs`, `src/model/graph.rs`, `src/rebuild/session.rs`, tests in `src/tui/consistency_tests/forms.rs`. Docs: `docs/operations.md` (forms paragraph and "What changed in the TUI"), `CHANGELOG.md` (Fixed), five rows in `docs/plans/tui-framework/operator-review.md`.

Verification: 9 consistency tests through `Msg::OpenNewMuxForm` and `Msg::OpenMuxLaunchForm`, plus 6 unit tests of `next_in_series`. The isolated suite passed 4864, failed 0, ignored 1. fmt, clippy (workspace with all features, and `tuikit` alone), rustdoc with `-D warnings`, `cargo build --examples`, and `git diff --check` all pass.

Snapshot evidence (`conspectus tui --snapshot`, 120x40 unless noted, name row of the form):
- showcase, Mux view, `bare-work` (no digits), `n` and `N`: before `name  -` (empty), after `name  bare-work-2`.
- showcase copy with `bare-work` renamed `worker-1` and the stopped `docs-sweep` renamed `worker-2`, Mux view `worker-1`, `n` and `N` (and `n` at 60 columns): before empty, after `worker-3`. History view `worker-2`, `n`: after `worker-3`.
- showcase, Sessions view first row (a repo group), `n`: empty before and after.
The cursor sits at the end of the seeded name.

**Regressions**
- A form opened from a mux, a session in one, or a mux pin is no longer empty. An operator who wants an unrelated name clears it first (`Ctrl-U`).

**Consistency wins**
- Both new-mux forms seed their name the same way, from the same entity the other accelerators act on, including the right pane's while it has focus.
- The TUI's seed and the rebuild's new-mux naming use one rule for a taken name.
<!-- SECTION:FINAL_SUMMARY:END -->
