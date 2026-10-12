---
id: CSP-587
title: Give the showcase fixture's sessions transcripts
status: To Do
assignee: []
created_date: '2026-10-12 13:55'
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
- [ ] #1 With a showcase session selected, `conspectus tui --snapshot --snapshot-fixture tests/fixtures/showcase.json` previews a transcript
- [ ] #2 The README frame is regenerated from the fixture and shows that transcript
<!-- AC:END -->

## Definition of Done
<!-- DOD:BEGIN -->
- [ ] #1 `just check` passes, or `git diff --check` for docs-only changes
- [ ] #2 Docs and ADRs are updated when behavior, architecture, or workflow change
<!-- DOD:END -->
