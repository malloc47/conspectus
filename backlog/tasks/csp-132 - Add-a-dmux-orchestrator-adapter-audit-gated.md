---
id: CSP-132
title: Add a dmux orchestrator adapter (audit-gated)
status: To Do
assignee: []
created_date: '2026-05-18 14:50'
labels:
  - h-agentmux
milestone: m-11
dependencies:
  - CSP-131
ordinal: 236000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
- Scope: **placeholder — may be closed as won't-do.** dmux's
  on-disk state layout is not surfaced in its README, so the audit
  in `CSP-131` is responsible for source-inspecting dmux
  and determining whether it tracks anything beyond a MUXPROC
  subset (task / feature metadata, agent lifecycle state, lineage
  between dmux-spawned sessions, container isolation). If the
  audit returns "MUXPROC subset," close this item. Otherwise
  implement the adapter against the surviving non-overlap evidence
  only — do not duplicate the pane ↔ harness link.
- Tests: deferred until the audit determines scope.
- Manual checks: run against a real dmux install if available;
  otherwise rely on fixtures captured from upstream.
- Blockers: `CSP-131` (audit must justify the work and
  define the evidence set).
<!-- SECTION:DESCRIPTION:END -->

Legacy ID: `H-AGENTMUX-005`
