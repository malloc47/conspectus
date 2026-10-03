---
id: CSP-038
title: 'Generate session, workspace, fork, and mux candidate links'
status: Done
assignee: []
created_date: '2026-05-15 20:50'
labels:
  - p3
milestone: m-4
dependencies:
  - CSP-035
  - CSP-037
ordinal: 38000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
- Scope: emit candidate links for session cwd/root matches, fork
  associations, mux candidates, parent session evidence, child session
  evidence, and unresolved lineage endpoints while preserving all plausible
  mux links.
- Tests: graph-fragment tests for orphan sessions, mux-only sessions,
  one-to-many mux candidates, fork-linked sessions, and unresolved lineage.
- Manual checks: inspect JSON to confirm ambiguous mux evidence remains in
  `candidate_links`.
- Blockers: `CSP-035`, `CSP-037`.
<!-- SECTION:DESCRIPTION:END -->

## Final Summary

<!-- SECTION:FINAL_SUMMARY:BEGIN -->
Added a `discovery::cross_link::infer` post-merge pass that derives
`AgentSession`→`MuxSession` `LinkedToMux` candidates (StrongDiscovered for
exact cwd matches, Discovered for prefix matches) and
`AgentSession`→`Fork` `AssociatedWith` candidates whenever a session cwd
sits at or below an atelier `RootedAtPath` fork root; every plausible mux
match is preserved and atelier-emitted `ParentSession`/`ChildSession`
unresolved lineage links pass through untouched.
<!-- SECTION:FINAL_SUMMARY:END -->

Legacy ID: `P3-007`
