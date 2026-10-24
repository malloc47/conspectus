---
id: CSP-170
title: Implement resume un-muxed agent session into mux
status: Done
assignee: []
created_date: '2026-05-19 03:51'
updated_date: '2026-10-22 15:06'
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

Legacy ID: `P8-011`

## Final Summary

<!-- SECTION:FINAL_SUMMARY:BEGIN -->
Covered by CSP-673 (rebuild a session by resuming its lineage head into its last mux or a new one); no separate work remains.
<!-- SECTION:FINAL_SUMMARY:END -->
