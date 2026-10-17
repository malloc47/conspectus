---
id: CSP-606
title: Choose a session-to-pane attribution source for opencode 2.x
status: To Do
assignee: []
created_date: '2026-10-17 02:59'
updated_date: '2026-10-17 03:03'
labels:
  - opencode-v2
milestone: m-21
dependencies:
  - CSP-602
  - CSP-605
references:
  - 'https://anoma.ly/notes/opencode-2-0/'
priority: medium
ordinal: 655000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
In 1.x the plugin's hook records were the strongest opencode session-to-pane evidence. CSP-221, an HTTP/ACP server adapter, was closed as won't-do because the in-process plugin already gave pane-accurate records, and its closure note says to reopen if that stops holding. In 2.x it stops holding. Plugins run in one shared service (CSP-605 makes those records safe), so a hook can no longer say which pane a session is open in. Fd evidence (`ActivePaneFdSessionMatch`) may weaken the same way, since the service rather than the pane's TUI process probably holds the database open. Without a replacement, attribution for 2.x falls back to activity timing and cwd, which already mis-binds pins when several panes share a repo.

The V2 server API is new (`@opencode/client`, with `Service.ensure()` autodiscovering the local service). It exposes session and tab operations, so it may be able to answer "which client has which session open". This is the reconsideration of CSP-221 that its closure note anticipated.

- Scope: compare the candidate sources CSP-602 turns up: a read-only V2 server API query mapping clients or tabs to sessions and client pids to panes; client identity carried in plugin event payloads; client argv or open files; activity-only fallback. Decide in an ADR that cites the ADR 0087 mutation envelope and ADR 0028 (the query must not create sessions, change recency, or send input), then implement the chosen source with a resolver rank relative to the existing evidence (ADR 0048 ordering).
- Tests: fake-service or fixture tests for one client, several clients in different panes, a client with no tmux, an unreachable service, and an ambiguous response.
- Manual checks: two sandboxed 2.x TUIs in two panes on the same repo each bind to their own session; querying does not change either session's recency.
- Blockers: CSP-602, CSP-605.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 An ADR records the chosen 2.x attribution source, the alternatives, and its resolver rank
- [ ] #2 Two opencode 2.x TUIs in different panes on the same repo resolve to their own sessions
- [ ] #3 The attribution path performs no writes and does not alter opencode session state or recency
- [ ] #4 With the service unreachable, discovery degrades to the remaining evidence without errors in the graph
<!-- AC:END -->

## Definition of Done
<!-- DOD:BEGIN -->
- [ ] #1 `just check` passes, or `git diff --check` for docs-only changes
- [ ] #2 Docs and ADRs are updated when behavior, architecture, or workflow change
<!-- DOD:END -->
