---
id: CSP-842
title: Name running agents without live state in `hook status`
status: To Do
assignee: []
created_date: '2026-11-12 08:07'
labels:
  - live-state
  - ls-cli
milestone: m-26
dependencies: []
priority: low
ordinal: 927000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
`hook status <harness>` ends with one live-state health line. It says `receiving` as soon as the live-state store holds any entry for the harness, so an agent whose hooks are silent (a workspace whose trust prompt was never accepted, an agent started before `hook init`) goes unnoticed while another agent of the same harness reports. Only when the store holds nothing for the harness does the line say `silent`. The evidence to tell the two apart already exists: the main-agent runtime processes in the saved graph and the pids the store records.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 With entries from one agent of a harness and another running agent of it that has no entry, the health line says how many running agents have no live state
- [ ] #2 An agent whose entry records a different pid than its runtime process (a wrapper process) is not reported as silent when the session matches
- [ ] #3 `hook status` still starts no process, asks no daemon, and stays as fast
- [ ] #4 Tests cover the mixed case and the operations guide describes it
<!-- AC:END -->

## Definition of Done
<!-- DOD:BEGIN -->
- [ ] #1 `just check` passes, or `git diff --check` for docs-only changes
- [ ] #2 Docs and ADRs are updated when behavior, architecture, or workflow change
<!-- DOD:END -->
