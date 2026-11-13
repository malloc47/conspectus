---
id: CSP-850
title: 'Pi: let the operator name the tools that count as subagents'
status: To Do
assignee: []
created_date: '2026-11-12 12:05'
labels:
  - live-state
  - ls-emit
milestone: m-26
dependencies: []
priority: low
type: enhancement
ordinal: 935000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
The Pi extension counts a session's subagents as the in-flight calls of tools whose name is in a fixed set, `{"subagent"}`, the name of Pi's reference subagent example (ADR 0206 decision 25). Pi has no subagent concept of its own, so a third-party subagent package with other tool names shows its children as ordinary tool calls. Research note 04 (open question 3) asks whether the set should be configurable in Conspectus's config and passed to the extension, through an environment variable or a regenerated extension file; either choice affects how `hook status pi` tells an installed copy from an outdated one, since status compares the file byte for byte. Found while building the Pi slice (CSP-817).
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 An operator can add tool names the Pi extension counts as subagent calls without editing the installed file by hand
- [ ] #2 The mechanism, and what hook status pi reports for a configured copy, are recorded in an ADR and in docs/operations.md
- [ ] #3 The extension's node tests and the shared event scripts cover a configured name
<!-- AC:END -->

## Definition of Done
<!-- DOD:BEGIN -->
- [ ] #1 `just check` passes, or `git diff --check` for docs-only changes
- [ ] #2 Docs and ADRs are updated when behavior, architecture, or workflow change
<!-- DOD:END -->
