---
id: CSP-848
title: Report opencode's live-state health in hook status
status: To Do
assignee: []
created_date: '2026-11-12 11:18'
labels:
  - live-state
  - ls-cli
milestone: m-26
dependencies: []
priority: low
type: enhancement
ordinal: 933000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
`conspectus hook status` takes only claude-code, codex, and pi, so the live-state health line (receiving, silent, no-events, off) is not available for opencode, whose plugin reports live state since CSP-816; the operations guide points operators at `table sessions --columns +live` instead. The store entries alone can say receiving. Saying installed or off means reading opencode's plugin config (`opencode.json`, `opencode.jsonc`, `cli.json`; JSONC), which ties into the open question of a `hook init opencode` (research note 03, question 8).
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 conspectus hook status opencode ends with the live-state health line
- [ ] #2 It writes nothing and reads no opencode file beyond the plugin config it needs
- [ ] #3 docs/operations.md documents it
<!-- AC:END -->

## Definition of Done
<!-- DOD:BEGIN -->
- [ ] #1 `just check` passes, or `git diff --check` for docs-only changes
- [ ] #2 Docs and ADRs are updated when behavior, architecture, or workflow change
<!-- DOD:END -->
