---
id: CSP-872
title: Bound the daemon socket connect when the listener's backlog is full
status: To Do
assignee: []
created_date: '2026-11-13 19:46'
labels:
  - live-state
  - ls-fix
milestone: m-26
dependencies: []
priority: low
ordinal: 952000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Every daemon socket request now times out on its reads and writes (ADR 0219), but `UnixStream::connect` itself has no bound. A daemon whose accept loop has stopped (the process stopped with SIGSTOP, say) still lets connections queue in the listener's backlog, so calls time out on their first read; once enough abandoned connections fill the backlog, `connect` blocks and the bound never starts. Surfaced while bounding the socket clients (CSP-836).
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 A client call against a listener whose backlog is full ends within its request's bound
- [ ] #2 A test fills a listener's backlog and shows the call returns
<!-- AC:END -->

## Definition of Done
<!-- DOD:BEGIN -->
- [ ] #1 `just check` passes, or `git diff --check` for docs-only changes
- [ ] #2 Docs and ADRs are updated when behavior, architecture, or workflow change
<!-- DOD:END -->
