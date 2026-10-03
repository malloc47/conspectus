---
id: CSP-378
title: Pin diagnostic surfaces in the TUI
status: Done
assignee: []
created_date: '2026-06-04 21:32'
labels:
  - h-pin
milestone: m-11
dependencies:
  - CSP-376
  - CSP-364
ordinal: 311000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
- Scope: each pin diagnostic gets a specific affordance — status
  bar text for unbound (`Enter to launch`), stale-mux (`Enter to
  relaunch in existing mux`), ambiguous (`b to bind`), drift
  (advisory). Right-pane detail surfaces the diagnostic plus, for
  ambiguous, the list of competing `agent_session_id`s.
- Tests: snapshot tests for each diagnostic state; reducer test
  for the `b` accelerator routing to the bind picker.
- Blockers: `CSP-376`, `CSP-364`.
<!-- SECTION:DESCRIPTION:END -->

## Final Summary

<!-- SECTION:FINAL_SUMMARY:BEGIN -->
Selected pin rows and bound pinned sessions now derive
status-bar hints from resolver diagnostics: unbound pins advertise
launch, stale mux pins advertise relaunch, ambiguous bindings
advertise `b` for bind guidance, and cwd drift is marked
advisory. Pin rows render a right-pane diagnostic preview, bound
agent-session details add pin diagnostic fields with competing
session ids for ambiguous bindings, and the `b` accelerator routes
to a bind command hint until the full picker lands in `CSP-389`.
<!-- SECTION:FINAL_SUMMARY:END -->

Legacy ID: `H-PIN-018`
