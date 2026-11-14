---
id: CSP-847
title: 'OpenCode 2.x: re-read pending permission asks when the server plugin starts'
status: Done
assignee:
  - '@claude'
created_date: '2026-11-12 11:17'
updated_date: '2026-11-14 00:26'
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
- [x] #1 A server entrypoint started while a permission ask is pending reports that session waiting, on an opencode whose server plugin context lists the location's sessions (ctx.session.list; the 2.0.32 types lack it, so that version reports it at the session's next event as before)
- [x] #2 The seeding reads only ids and closed-set fields of the listed asks
- [x] #3 A test with a stand-in plugin context covers it
<!-- AC:END -->

## Definition of Done
<!-- DOD:BEGIN -->
- [x] #1 `just check` passes, or `git diff --check` for docs-only changes
- [x] #2 Docs and ADRs are updated when behavior, architecture, or workflow change
<!-- DOD:END -->

## Implementation Plan

<!-- SECTION:PLAN:BEGIN -->
1. Findings: the 2.0.32 server plugin context has `ctx.permission.list({sessionID})` (sessionID required) but no form domain; its `session` domain lacks `list`, which the current server-plugin docs show (`ctx.session.list({directory, limit})`, cursor pages). So the seeding lists the location's most recent sessions where the context can, and asks each for its pending permissions; forms are not re-read.
2. Reducer (pure, live.ts): `seed(sessionID, listed, now)` adds pending asks the instance never saw (reads only an ask's id and session, a form's kind, a status word), never removes, and skips asks it saw end (a bounded set of ended keys, so a list read before an answer cannot revive it). `readPending(api, directory, each)` drives the two calls defensively (any failure skips).
3. Server entrypoint: after subscribing, run readPending and feed each session's list to seed.
4. Tests: a stand-in context test of readPending (with a proxy proving which fields are read), an entrypoint test that loads server.ts under node with a stand-in context and writer, and a shared event script (v2-seed-pending) replayed by the plugin tests and the Rust tests.
5. Docs: ADR (what is re-read and what cannot be), README, operations guide, CHANGELOG; plugin 0.4.0.
<!-- SECTION:PLAN:END -->

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
Findings from @opencode/plugin 2.0.32's published types (fetched from unpkg) and the server plugin docs: ctx.permission.list({sessionID}) requires a session id; the server context has no form domain; its session domain lacks `list` in 2.0.32 but the current docs show ctx.session.list({directory, limit}) with cursor pages. So the seeding lists up to 100 recent sessions where it can and asks each; forms are not re-read. ADR 0228.
Validation: just opencode-plugin-test 33/33 (seed field-read proxy test, readPending stand-in context tests, server.ts loaded under node against a stand-in context and writer, script v2-seed-pending); Rust replay v2_asks_pending_when_the_server_plugin_starts; checks.sh green; just check 5683 passed, 1 skipped. server.ts and write.ts now run under node's type stripping in the entrypoint test; tsc was not run (no npm install offline).
<!-- SECTION:NOTES:END -->

## Final Summary

<!-- SECTION:FINAL_SUMMARY:BEGIN -->
The opencode 2.x server entrypoint (plugin 0.4.0) now reads permission asks that were pending when it started: after subscribing to its events it lists its location's 100 most recent sessions (`ctx.session.list`, cursor pages of 50) and asks each for its pending permissions (`ctx.permission.list({sessionID})`). The reducer's new `seed` adds the asks it never saw (session reads `waiting/permission`, a subagent's ask raises its root), reads only an ask's id and session (plus a form's kind and a status word for callers that have them), never removes a wait, and skips asks it saw end (the last 512), so a list read before an answer cannot revive one. Any missing call or failure is skipped silently. Limits: the 2.0.32 server context has no `session.list` (only newer docs show it), and no form list exists for server plugins, so forms are not re-read. ADR 0228; README, operations guide, fixture README, CHANGELOG updated.

Verified with `just opencode-plugin-test` (33 pass, including a proxy test of the fields read, stand-in context tests of the lists, a test that loads `server.ts` under node against a stand-in context and writer, and script `v2-seed-pending`), its Rust replay, `/tmp/csp-fu/checks.sh`, and `just check` (5683 passed, 1 skipped). `tsc` was not run (no `npm install` offline).
<!-- SECTION:FINAL_SUMMARY:END -->
