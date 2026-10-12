---
id: CSP-587
title: Give the showcase fixture's sessions transcripts
status: Done
assignee:
  - '@claude'
created_date: '2026-10-12 13:55'
updated_date: '2026-10-12 18:28'
labels:
  - rel
milestone: m-20
dependencies: []
priority: medium
ordinal: 636000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
- Why: ADR 0112 makes a session's Preview its transcript. The sessions in `tests/fixtures/showcase.json` point at `/fixture/...` state roots with no transcripts behind them, so a snapshot of the showcase reads "transcript unavailable: transcript file not found". The README frame rendered from that fixture still shows the old "loading mux preview…", and regenerating it today would show the unavailable state instead of a conversation.
- Open question: how a checked-in fixture points at checked-in transcripts (paths resolved against the fixture's directory, or transcripts carried in the fixture).
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [x] #1 With a showcase session selected, `conspectus tui --snapshot --snapshot-fixture tests/fixtures/showcase.json` previews a transcript
- [x] #2 The README frame is regenerated from the fixture and shows that transcript
<!-- AC:END -->

## Definition of Done
<!-- DOD:BEGIN -->
- [x] #1 `just check` passes, or `git diff --check` for docs-only changes
- [x] #2 Docs and ADRs are updated when behavior, architecture, or workflow change
<!-- DOD:END -->

## Implementation Plan

<!-- SECTION:PLAN:BEGIN -->
1. ADR 0113: PNG screenshots via `termshot` from `tui --snapshot` frames of the showcase fixture; previews read from a materialized showcase world; pinned clock.
2. Showcase scenario: conversation, dated git history with work in progress, recorded pane; `dev scenario materialize`; scenario TUI answers panes from recordings.
3. Snapshot flags `--snapshot-fixture-root` and `--snapshot-now`; regenerate the fixture.
4. `just screenshots`, `termshot` in the dev shell, README images, dev-scenarios docs; a pipeline test.
<!-- SECTION:PLAN:END -->

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
- Open question answered (ADR 0113): the fixture keeps its `/fixture/...` paths, and `--snapshot-fixture-root` reads previews from a world `dev scenario materialize` writes, rather than checking transcripts into `tests/fixtures/`. So AC 1 holds with `--snapshot-fixture-root`; a bare `--snapshot-fixture` still previews "transcript unavailable", by design.
- Per the operator, everything the screenshots show lives in the showcase scenario itself, so `dev scenario tui showcase` shows the same conversation, git state, and pane.
- `termshot`'s font lacks `⚠`, which renders as a box; the README caption says so.
- Tried `freeze` first; it crashed or hung on a full frame.
<!-- SECTION:NOTES:END -->

## Final Summary

<!-- SECTION:FINAL_SUMMARY:BEGIN -->
The README now shows four PNG screenshots of the TUI on the showcase world (session transcript, Related-row evidence over git status, repo history, mux pane), rendered by `just screenshots` with `termshot` (ADR 0113). The showcase scenario carries a conversation, a dated git history with work in progress, and a recorded pane; `dev scenario materialize` keeps its world on disk, and the dev-only `--snapshot-fixture-root` and `--snapshot-now` flags read previews from it and pin ages. Verified with `just check`, a binary-level pipeline test of all four screens, unit tests for materialize and the recorded pane, and by viewing the rendered images.
<!-- SECTION:FINAL_SUMMARY:END -->
