---
id: CSP-134
title: Add an agent-of-empires orchestrator adapter (audit-gated)
status: To Do
assignee: []
created_date: '2026-05-18 14:50'
labels:
  - h-agentmux
milestone: m-11
dependencies:
  - CSP-131
ordinal: 238000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
- Scope: **placeholder — may be closed as won't-do.** The
  strongest theoretical edge over MUXPROC is container isolation:
  when agent-of-empires runs the agent inside a container, the
  host process tree shows only the runtime
  (`docker`/`podman`/`bwrap`) and MUXPROC cannot identify the
  harness. The audit in `CSP-131` should determine (a)
  whether agent-of-empires actually tracks the in-container agent
  identity in host-visible state, and (b) whether container
  isolation is in Conspectus's near-term scope at all. If both
  are yes, implement an adapter scoped to container-isolated
  sessions and any other surviving non-overlap evidence; emit
  `AgentSession` nodes carrying the orchestrator-known harness
  key without pretending Conspectus's harness adapters can parse
  their transcripts.
- Tests: deferred until the audit determines scope.
- Manual checks: run against a real agent-of-empires install if
  available.
- Blockers: `CSP-131`.
<!-- SECTION:DESCRIPTION:END -->

Legacy ID: `H-AGENTMUX-007`
