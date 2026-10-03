---
id: CSP-348
title: Model blocked-session metadata
status: To Do
assignee: []
created_date: '2026-06-03 12:44'
labels:
  - h-continue
milestone: m-11
dependencies:
  - CSP-347
ordinal: 226000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
- Scope: add optional metadata to `AgentSessionNode` or a typed
  sidecar record that captures `blocked_reason`, parsed
  `resume_after_epoch`, the source message snippet, parser
  confidence, and harness/source provenance. Keep ordinary sessions
  byte-stable by skipping absent fields in JSON.
- Tests: serde round trips, sparse-session JSON snapshots, and
  no-field output for sessions without a recognized usage-limit
  tail.
- Blockers: `CSP-347`.
<!-- SECTION:DESCRIPTION:END -->

Legacy ID: `H-CONTINUE-002`
