---
id: CSP-910
title: >-
  TUI: the mux that hosts this TUI previews its agent transcript under
  mux_preview = transcript
status: Done
assignee:
  - '@claude'
created_date: '2026-11-25 18:58'
updated_date: '2026-11-25 19:28'
labels:
  - tui
  - config
dependencies: []
references:
  - src/tui/preview/mod.rs
  - >-
    docs/adr/0256-tui-a-live-mux-can-preview-the-transcript-of-the-agent-it-hosts.md
priority: low
type: enhancement
ordinal: 985000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
The mux a TUI runs in previews nothing: its row says `no mux target for this row`, because capturing its pane would show the TUI inside itself. CSP-908 added `[tui] mux_preview = "transcript"`, which makes a live mux preview the transcript of the agent it hosts, and kept this exception as it was (ADR 0256, decision 7). That recursion hazard does not apply to a transcript, so an operator who runs an agent in the same tmux session as Conspectus, and opted in to transcripts, still gets an empty Preview on that one mux where every other mux shows its agent.

It was left out of CSP-908 on purpose: it changes a documented rule (ADR 0112, ADR 0253, ADR 0256) for a case that opting in does not need, so it should be asked for. The operator asked for it on 2026-11-25.

Two points to settle in the work. Under `pane`, nothing can change, since the pane is the recursion. Without a live agent whose harness the viewer can read, the mux has nothing safe to show and keeps its placeholder, which should still say something true.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [x] #1 Under `mux_preview = "transcript"`, the mux that hosts this TUI previews the transcript of the session `v` opens on it, in every place a live mux previews: the Mux view row, the Pinned view row for a pin on it, a drilled-in node, and a Related neighbor
- [x] #2 Without a live agent whose harness the viewer can read, and under `pane`, that mux previews nothing as it does today, with a placeholder that is still accurate
- [x] #3 Attaching to the mux that hosts this TUI is still refused, and a pane of it is still never captured
- [x] #4 A new ADR records the change to the rule and amends ADR 0256; the operator guide and design.md say so
- [x] #5 Tests cover the source in each place above, both sparse cases, and a rendered frame; `just check` passes
<!-- AC:END -->

## Definition of Done
<!-- DOD:BEGIN -->
- [x] #1 `just check` passes, or `git diff --check` for docs-only changes
- [x] #2 Docs and ADRs are updated when behavior, architecture, or workflow change
<!-- DOD:END -->

## Implementation Plan

<!-- SECTION:PLAN:BEGIN -->
1. Resolver (`src/tui/preview/mod.rs`): the mux that hosts this TUI resolves to `hosted_transcript` (never a pane), in `node_source`'s mux arm and in `selection_source`'s mux-row arm, where attaching to it fails so the pane path yields nothing. Under `pane`, or with no live readable agent, it still resolves to none. `resolve_attach_target` is untouched, so attaching stays refused.
2. Tests (red-check against the old behavior): the source in each place (Mux row, Pinned row, drilled-in node, Related neighbor), both sparse cases and `pane`, attach still refused and no `Pane` source for the hosting mux, `--no-live-preview`, and a rendered frame. Rewrite the assertion in `the_transcript_choice_leaves_other_rows_as_they_were` that pinned the old behavior.
3. Docs: ADR 0258 amending ADR 0256 decision 7, the ADR index and ADR 0256's status line, the operations guide, design.md, changelog, and the doc comments that say a hosting mux previews nothing.
4. Check a real frame with `tui --snapshot`, then `just check`.
<!-- SECTION:PLAN:END -->

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
Where it landed: `hosting_mux_source` and a `hosts_this_tui` arm in `node_source` (`src/tui/preview/mod.rs`). The Mux-view row needed its own change, since attaching to the TUI's own session fails and the pane path then yields nothing: `selection_source`'s mux-row arm now falls back to `hosting_mux_source` before the last-hosted fallback. `resolve_attach_target` is untouched, so attach stays refused, and no path builds a `Pane` source for the hosting mux.
The place tests were refactored onto one helper, `live_mux_places` (Pinned row, Mux row, Related neighbor), shared by the CSP-908 tests and these; the old assertion that pinned the hosting mux to nothing is gone.
Red-checks: with the hosting mux resolving to nothing again, the three behavior tests fail and the three guards (pane choice, no agent, attach and capture) pass; with only the `selection_source` fallback removed, the Mux place fails, so that arm is needed.
Not covered: the real `$TMUX` probe that fills `current_tmux_session`. The tests set that field directly; the probe already fed the attach guard and the old "previews nothing" rule, and an end-to-end test needs the tmux harness's full stub world with an agent transcript.
Unchanged and noted: the placeholder for a hosting mux with nothing to show is still `no mux target for this row`, which describes it loosely (ADR 0258 lists a clearer string as a separate change). A session row under `session_preview = "pane"` whose mux hosts this TUI resolves to no preview in `selection_source`, unlike in `node_source`, which falls back to the transcript; that looks like an older inconsistency, untouched here.
Validation: `just check` green (6054 passed, 1 skipped; fmt, clippy, rustdoc -D warnings, diff check; tuikit-features skipped, nothing under crates/tuikit changed). Not run locally: CI's `cargo test --workspace`.
<!-- SECTION:NOTES:END -->

## Final Summary

<!-- SECTION:FINAL_SUMMARY:BEGIN -->
Under `[tui] mux_preview = "transcript"`, the tmux session Conspectus runs in now previews the transcript of the agent it hosts (the one `v` opens on it) in every place a live mux previews: the Mux view's row, the Pinned view's row for a pin on it, a drilled-in mux, and a Related row's neighbor. Its pane is still never previewed or captured and attaching to it is still refused; under `pane`, or with no live agent whose harness the viewer can read, the row says what it said before. ADR 0258 records the change to ADR 0256's decision 7; the ADR index, operations guide, design.md, and changelog are updated.
Verified with 6 new tests (each place, the pane choice, three sparse cases, attach and capture, `--no-live-preview`, a rendered frame; the three behavior tests fail on the old code) and `just check` (6054 passed, 1 skipped; fmt, clippy, rustdoc, diff check). The real `$TMUX` probe is not exercised: tests set `current_tmux_session` directly. Not run locally: CI's `cargo test --workspace`.
<!-- SECTION:FINAL_SUMMARY:END -->
