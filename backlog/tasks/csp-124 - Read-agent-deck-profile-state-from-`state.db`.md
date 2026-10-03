---
id: CSP-124
title: Read agent-deck profile state from `state.db`
status: To Do
assignee: []
created_date: '2026-05-17 20:45'
labels:
  - h-agentmux
milestone: m-11
dependencies:
  - CSP-131
  - CSP-122
ordinal: 234000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
- Scope: agent-deck stores richer per-session metadata
  (`~/.agent-deck/profiles/<profile>/state.db`, SQLite). **This
  item is gated on the audit in `CSP-131` confirming the
  SQLite content includes evidence MUXPROC cannot supply** —
  plausible candidates are profile labels / tags, agent lifecycle
  state for exited or paused sessions, and session-to-multi-repo
  binding when the agent has since changed cwd. If the audit shows
  the SQLite content is only a snapshot of what MUXPROC already
  sees live, close this item as won't-do. Otherwise read the
  SQLite state read-only and emit only the surviving non-overlap
  evidence. Reuse the `rusqlite` dependency added by ADR 0013.
- Tests: fixture tests over a temp SQLite file populated with
  representative rows; missing-database degradation; malformed
  schema degradation.
- Manual checks: confirm read-only access; confirm the adapter
  does not lock the database while agent-deck is running.
- Blockers: `CSP-131` (audit must justify the work),
  `CSP-122`. Requires recording the agent-deck schema and
  the surviving evidence set in an ADR (or an extension of ADR
  0013) before introducing the read code.
<!-- SECTION:DESCRIPTION:END -->

Legacy ID: `H-AGENTMUX-004`
