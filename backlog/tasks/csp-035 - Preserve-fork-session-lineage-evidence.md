---
id: CSP-035
title: Preserve fork session lineage evidence
status: Done
assignee: []
created_date: '2026-05-15 20:50'
labels:
  - p3
milestone: m-4
dependencies:
  - CSP-028
  - CSP-034
ordinal: 35000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
- Scope: map native, approximate, unsupported, fresh, and not-yet-discovered
  lineage evidence from provider metadata into candidate links or unresolved
  endpoints without fabricating placeholder session nodes.
- Tests: unit tests for each lineage capability and unresolved parent/child
  session evidence per ADR 0005.
- Manual checks: inspect JSON for unresolved lineage evidence and confirm the
  evidence is preserved without fake nodes.
- Blockers: `CSP-028`, `CSP-034`.
<!-- SECTION:DESCRIPTION:END -->

## Final Summary

<!-- SECTION:FINAL_SUMMARY:BEGIN -->
Extended `fork_records_fragment` to emit `ParentSession` and
`ChildSession` candidate links with unresolved-endpoint evidence carrying
`harness_key`, `native_id`, fork root path, and a `lineage_kind` of
`native`/`approximate`/`unsupported`/`fresh` plus any
`degraded_warning`; capability maps to confidence (Native=High,
Approximate=Medium, Unsupported/Fresh=Low); fresh sessions without a
`source_session` omit the parent link rather than inventing one, and no
placeholder `AgentSession` nodes are emitted.
<!-- SECTION:FINAL_SUMMARY:END -->

Legacy ID: `P3-004`
