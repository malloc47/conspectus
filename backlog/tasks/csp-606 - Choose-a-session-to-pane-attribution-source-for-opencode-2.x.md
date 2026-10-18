---
id: CSP-606
title: Choose a session-to-pane attribution source for opencode 2.x
status: Done
assignee:
  - '@claude'
created_date: '2026-10-17 02:59'
updated_date: '2026-10-18 06:40'
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
- [x] #1 An ADR records the chosen 2.x attribution source, the alternatives, and its resolver rank
- [x] #2 Two opencode 2.x TUIs in different panes on the same repo resolve to their own sessions
- [x] #3 The attribution path performs no writes and does not alter opencode session state or recency
- [x] #4 With the service unreachable, discovery degrades to the remaining evidence without errors in the graph
<!-- AC:END -->

## Definition of Done
<!-- DOD:BEGIN -->
- [x] #1 `just check` passes, or `git diff --check` for docs-only changes
- [x] #2 Docs and ADRs are updated when behavior, architecture, or workflow change
<!-- DOD:END -->

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
Scope update from the CSP-602 spike (ADR 0125):
- The V2 server API has no read-only client-to-session mapping. `GET /api/session/active` lists only sessions with a running turn; `/api/event` carries no client identity; `POST /api/session/{id}/view` is a write. `latest/tui/tabs.json` is shared by every client. TUI clients hold no opencode.db fds, and their argv never follows in-app session switches.
- A 2.x TUI plugin runs inside each client process with that pane's `TMUX`/`TMUX_PANE`, and `ctx.ui.router.current()` gives the client's current session. That is the replacement attribution source: hook records written from the TUI entrypoint keep the existing hook rank. CSP-221's server adapter stays closed.

Decision (ADR 0126): the 2.x attribution source is the plugin's TUI entrypoint (CSP-607), not a server API query. CSP-602 found no read-only client-to-session mapping in the V2 API (`session.active` covers only running turns, the event stream has no client identity, `view` is a write), `tabs.json` is shared by all clients, TUIs hold no DB fds, and argv goes stale after in-app switches. The TUI entrypoint runs in the client's pane, so its records keep the existing hook rank (`hook_session_match`, above activity/cwd/launch-argv evidence per ADR 0048), and no new reader or resolver rank was needed. CSP-221 stays closed.
Tests adapt the task's list to that source: two TUI clients in different panes resolve to their own sessions; a client with no tmux context binds only through its own pid; one session open in two clients yields an active candidate per pane for the resolver; no records (plugin absent or service unreachable) leaves the graph to other evidence; service-context records never bind (CSP-605).
Manual check (see CSP-607 notes): two sandboxed 2.x TUIs on the same repo, one switching sessions in-app, each resolved to its own session via hook_sidecar. The entrypoint reads only client router state and never calls the server API, so querying alters no session state or recency.
<!-- SECTION:NOTES:END -->

## Final Summary

<!-- SECTION:FINAL_SUMMARY:BEGIN -->
Chose the Conspectus plugin's opencode 2.x TUI entrypoint as the session-to-pane attribution source (ADR 0126): it runs in each client's pane, so its hook records keep hook rank and follow in-app switches without any server API query, writes, or recency changes. Sidecar and resolver tests cover two clients in different panes, a client without tmux, one session in two clients, and no records; a sandboxed two-pane check resolved each 2.x session to its own pane.
<!-- SECTION:FINAL_SUMMARY:END -->
