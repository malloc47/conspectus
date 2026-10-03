---
id: CSP-170
title: Implement resume un-muxed agent session into mux
status: To Do
assignee: []
created_date: '2026-05-19 03:51'
labels:
  - p8
milestone: m-13
dependencies:
  - CSP-169
ordinal: 397000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
- Scope: model harness-specific resume command support for the
  discovered harnesses Conspectus can safely resume. Add a confirmation
  flow that creates or selects the mux target per the CSP-160 answer,
  launches the resume command, and refreshes the graph afterward.
  Unsupported harnesses must show a disabled action with the reason.
- Tests: fake harness-action tests for supported/unsupported harnesses,
  command construction, missing transcript/session state, mux creation
  failure, launch failure, and refresh-after-success. Include one
  integration-style test that verifies no resume command is offered when
  the graph evidence is ambiguous.
- Manual checks: select an un-muxed test session for each supported
  harness and verify it resumes in the expected mux target.
- Blockers: `CSP-169`; may require follow-up ADR if resume semantics
  differ materially by harness.
<!-- SECTION:DESCRIPTION:END -->

## Final Summary

<!-- SECTION:FINAL_SUMMARY:BEGIN -->
Outcome (2026-09-30, `CSP-538`): still open. `S` resumes an un-muxed
session in a new terminal; resuming it into a mux remains to do.
<!-- SECTION:FINAL_SUMMARY:END -->

Legacy ID: `P8-011`
