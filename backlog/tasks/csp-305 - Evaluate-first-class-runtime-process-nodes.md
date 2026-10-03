---
id: CSP-305
title: Evaluate first-class runtime process nodes
status: Done
assignee: []
created_date: '2026-05-30 23:01'
labels:
  - h-muxproc-fu
milestone: m-11
dependencies: []
ordinal: 249000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
- Scope: turn ADR 0047's proposed model into a concrete workstream
  proposal if process evidence continues to accumulate resolver,
  mux-cardinality, opencode subagent, server/proxy, or diagnostic
  responsibilities. Compare keeping process evidence in
  `GraphLink.source_metadata` against adding ephemeral
  `RuntimeProcess` nodes and explicit mux/process/session links.
- Deliverable: either reject process nodes with updated rationale,
  or split implementation into model/schema, discovery, resolver,
  query, and TUI/detail slices with migration and snapshot-impact
  notes.
- Tests: design-only until accepted. Any implementation should add
  fixtures for single-agent, multi-agent, subagent, stale argv, and
  unreadable process cases.
- Related: ADR 0047, ADR 0046, `CSP-219`, `CSP-298`.
- Blockers: none.
<!-- SECTION:DESCRIPTION:END -->

## Final Summary

<!-- SECTION:FINAL_SUMMARY:BEGIN -->
ADR 0047 is accepted. Runtime process nodes should land
before graph visualization exports so DOT/HTML designs are not
built around a process-free graph. Process observations remain
ephemeral and rebuildable, while `AgentSession -> MuxSession`
stays the main user-facing resolved relationship. Implementation
is split into the follow-up slices below.
<!-- SECTION:FINAL_SUMMARY:END -->

Legacy ID: `H-MUXPROC-FU-001`
