---
id: CSP-298
title: Suppress subagent sessions from mux attachment resolution
status: To Do
assignee: []
created_date: '2026-05-28 23:02'
labels:
  - h-subagent
milestone: m-11
dependencies:
  - CSP-296
ordinal: 354000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
- Scope: when a subagent session's cwd matches a mux session, the
  resolver should prefer the human parent session for mux attachment
  rather than the subagent itself. This prevents a subagent session
  from "stealing" the mux link from its parent. If no parent is
  discovered, a standalone subagent should resolve normally rather
  than being left unmuxed.
- Tests: resolver tests for subagent-with-parent (parent preferred),
  orphan subagent (resolves normally), and subagent-with-parent where
  the parent has a stronger non-CWD link (parent still preferred).
- Blockers: `CSP-296`, existing `LinkedToMux` resolver tests.
<!-- SECTION:DESCRIPTION:END -->

Legacy ID: `H-SUBAGENT-004`
