---
id: CSP-859
title: Update the snapshot key examples that walk onto a Related row
status: To Do
assignee: []
created_date: '2026-11-13 00:24'
labels:
  - live-state
  - ls-tui
milestone: m-26
dependencies: []
priority: low
ordinal: 944000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
The project instructions (CLAUDE.md and AGENTS.md), README.md, and docs/dev-scenarios.md give `--snapshot-pane right --snapshot-keys 'j<Tab>jjjjj'` as the way to walk the right pane's cursor past the Node fields onto a Related row. Since ADR 0213 (CSP-829) a session or mux with live state lists up to five more fields (`state`, `since`, `source`, `evidence`, `subagents`, or `agents` and `pane_agents`), so on such a node the example stops on a field instead of a Related row. The showcase screenshot recipe and test were updated; the prose examples were not, and CLAUDE.md is the operator's file to edit.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 Every documented example that claims to reach a Related row does so on a session with live state, or says how many fields it walks past
- [ ] #2 CLAUDE.md is changed only by the operator or with the operator's explicit approval
<!-- AC:END -->

## Definition of Done
<!-- DOD:BEGIN -->
- [ ] #1 `just check` passes, or `git diff --check` for docs-only changes
- [ ] #2 Docs and ADRs are updated when behavior, architecture, or workflow change
<!-- DOD:END -->
