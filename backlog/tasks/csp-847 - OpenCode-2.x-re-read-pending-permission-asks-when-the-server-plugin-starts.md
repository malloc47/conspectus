---
id: CSP-847
title: 'OpenCode 2.x: re-read pending permission asks when the server plugin starts'
status: To Do
assignee: []
created_date: '2026-11-12 11:17'
labels:
  - live-state
  - ls-emit
milestone: m-26
dependencies: []
priority: low
type: enhancement
ordinal: 932000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
In opencode 2.x permission asks and forms are ephemeral events (research note 03, sections 3.2 and 5.4). A server entrypoint that starts or reloads while a session waits on the operator never sees that `permission.asked`, so the session reads working, or nothing, until its next event; the operator's most wanted state is the one lost. The server plugin context offers `ctx.permission.list({sessionID})`; whether forms have an equivalent is unverified. Found while building the OpenCode slice (CSP-816), whose reducer starts empty.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 A server entrypoint started while a permission ask is pending reports that session waiting
- [ ] #2 The seeding reads only ids and closed-set fields of the listed asks
- [ ] #3 A test with a stand-in plugin context covers it
<!-- AC:END -->

## Definition of Done
<!-- DOD:BEGIN -->
- [ ] #1 `just check` passes, or `git diff --check` for docs-only changes
- [ ] #2 Docs and ADRs are updated when behavior, architecture, or workflow change
<!-- DOD:END -->
