---
id: CSP-862
title: Match the content filter by session key in table mux
status: To Do
assignee: []
created_date: '2026-11-13 02:51'
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

## Definition of Done
<!-- DOD:BEGIN -->
- [ ] #1 `just check` passes, or `git diff --check` for docs-only changes
- [ ] #2 Docs and ADRs are updated when behavior, architecture, or workflow change
<!-- DOD:END -->
