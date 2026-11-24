---
id: CSP-905
title: Preview a stopped mux as the session it hosted last
status: Done
assignee:
  - '@claude'
created_date: '2026-11-24 16:18'
updated_date: '2026-11-24 16:33'
labels:
  - tui
  - history
milestone: m-23
dependencies: []
priority: medium
type: enhancement
ordinal: 981000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Selecting a stopped mux, pinned or only in history, leaves the Preview with nothing to show even though the graph knows what the mux was doing. In the Mux view a pinned stopped mux says `no mux target for this row`, and in the History view the same mux says `no preview for this row`, while the fields above the Preview list `last hosted ● <session>` for it. The operator reported this on 2026-11-24: the Preview should show that session's rendered preview in place of the placeholder.

Today `node_source` returns no source for any node that isn't live (ADR 0140: a gone node has no pane, repository, or directory to read), and `selection_source` asks for a pane the stopped mux no longer has. The `mux_last_hosted` link from a stopped mux to its last session is already in the graph, projected from history, and the `last hosted` Related row and the Pinned view's session rows read it; the Preview does not.

Out of scope: a Pinned view row for a stopped pin keeps its pin summary (status and the `rebuild` hint), which is not a mux row.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [x] #1 Selecting a stopped or removed mux, pinned or not, in the Mux view shows the Preview of the session it hosted last in place of `no mux target for this row`
- [x] #2 The History view gives a mux row the same Preview, and a drilled-in mux node does too, so the same mux previews alike in every view
- [x] #3 The session previews as its own row would: its transcript, or its live pane when the session preview is set to pane and the session now runs in another live mux; `--no-live-preview` hides it as it hides a session's
- [x] #4 A stopped mux with no recorded last-hosted session, or whose session is itself gone or unknown to the graph, keeps today's placeholder
- [x] #5 A running mux previews exactly as before, its pane, and none when it hosts this TUI; the Pinned view's stopped pin rows keep their summary
- [x] #6 Tests cover the source for a stopped mux in each view, the sparse cases above, and a rendered frame; the docs that describe what the Preview shows for a mux say so
<!-- AC:END -->

## Definition of Done
<!-- DOD:BEGIN -->
- [x] #1 `just check` passes, or `git diff --check` for docs-only changes
- [x] #2 Docs and ADRs are updated when behavior, architecture, or workflow change
<!-- DOD:END -->

## Implementation Plan

<!-- SECTION:PLAN:BEGIN -->
1. Model: add `GraphSnapshot::last_hosted_session(&NodeId)` beside `mux_host_of`, its inverse. It returns the agent session a mux's `mux_last_hosted` link points to, only if that session is in the snapshot and live; with several, the most recently active. Read from candidate links, as the `last hosted` Related row and `rows/pinned.rs` do (the resolver doesn't promote this relation).
2. Preview source (`src/tui/preview/mod.rs`): a `last_hosted_source` helper for a mux that is not live: it previews `node_source` of its last-hosted session, so it follows that session's own rule (transcript, or the live pane under `session_preview = pane`; hidden by `--no-live-preview`; none if the session is gone). Call it from the not-live early return in `node_source` (covers the History view and a drilled-in node) and as the fallback of `selection_source`'s mux-row arm when no pane can be attached (the Mux view and mux headers elsewhere). A running mux is untouched, including one that hosts this TUI. Pin rows are untouched.
3. Keep `sourceless_preview` for the sparse cases (no link, session gone), so today's placeholders still say something true.
4. Tests: the model helper (link present, no link, session missing or gone, several); Preview source for a stopped mux in the Mux view, the History view, and drilled in; running mux and hosting-this-TUI unchanged; pinned and unpinned; `session_preview = pane`; `--no-live-preview`; sparse cases; a rendered frame showing the transcript and not the placeholder. Update `a_ghost_mux_has_no_pane_to_preview_but_a_live_one_does`, whose claim is now narrower. Red-check the new tests against the old behavior.
5. Docs: a new ADR (0253) recording that a stopped mux previews its last-hosted session, with ADR 0112's status line and the README index updated; a CHANGELOG entry; `docs/design.md` if it states what a mux previews; the doc comment on `node_source`.
6. Check the real frames with `tui --snapshot` over a scratch fixture before and after. Then `just check`.
<!-- SECTION:PLAN:END -->

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
Where the placeholder came from: `node_source` returned nothing for any node that is not live, and `selection_source` asked `resolve_attach_target` for a pane a stopped mux no longer has. Fix: `GraphSnapshot::last_hosted_session` (the inverse of `mux_host_of`, from the `mux_last_hosted` candidate link, as the `last hosted` Related row reads it) and `preview::last_hosted_source`, called from `node_source`'s not-live branch and as the fallback of `selection_source`'s mux-row arm. It returns `node_source` of the session, so the session's own rules apply (transcript, pane under `session_preview = pane`, `--no-live-preview`, none if gone).
Decisions: pin rows are untouched, so the Pinned view's stopped pin keeps its summary and `rebuild` hint (a test pins that); a new ADR (0253) records the rule since it amends ADR 0112's table, with 0112's status line, the ADR index, `docs/design.md`, and the CHANGELOG updated. The shared showcase fixture was not changed (its frames feed doc tests); the tests build their own world through the real history projection.
Test findings worth keeping: the resolver overrides a second session's `linked_to_mux` to a mux another session already claims, so the pane-preference test moves the session into a mux of its own; and under `--no-live-preview` the old placeholder reads the same as the new hidden-session text, so that test also asserts the source.
Validation: `just check` passed (5991 tests, 1 skipped; fmt, clippy, rustdoc -D warnings, diff check; `tuikit-features` skipped, nothing under `crates/tuikit` changed), and fmt, clippy, and the diff check were re-run after the last test was added. 14 new tests: 6 model, 8 Preview; with `last_hosted_source` disabled, 5 of the Preview tests failed (the other new ones guard unchanged behavior). Real frames via `tui --snapshot` over a scratch fixture and a materialized showcase world: the History row for the stopped `release-prep` showed `showcase-claude`'s rendered conversation under its `last hosted` row, where it said `no preview for this row`. Not run locally: CI's second pass, `cargo test --workspace`.
<!-- SECTION:NOTES:END -->

## Final Summary

<!-- SECTION:FINAL_SUMMARY:BEGIN -->
A stopped mux, pinned or only remembered, now previews the session it hosted last in place of `no mux target for this row` (Mux view) and `no preview for this row` (History). The session previews as its own row would: its transcript, its pane under the pane setting once it runs in another mux, hidden by `--no-live-preview`. A stopped mux with no session to show, or whose session is gone, keeps its placeholder; a running mux and a stopped pin in the Pinned view are unchanged. Added `GraphSnapshot::last_hosted_session` and `preview::last_hosted_source`; ADR 0253 records the rule (amending ADR 0112), with design.md and the CHANGELOG updated.
Verified with 14 new tests (5 of the Preview ones fail with the fix disabled), `just check` (5991 passed), and a rendered frame of a stopped mux previewing its session's conversation from a materialized showcase world. Work is on branch `feat/stopped-mux-previews-last-hosted-session`, uncommitted.
<!-- SECTION:FINAL_SUMMARY:END -->
