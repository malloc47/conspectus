---
id: CSP-846
title: 'OpenCode 2.x: live state from a TUI-only plugin install'
status: Done
assignee:
  - '@claude'
created_date: '2026-11-12 11:17'
updated_date: '2026-11-14 00:33'
labels:
  - live-state
  - ls-emit
milestone: m-26
dependencies: []
priority: low
type: enhancement
ordinal: 931000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
On opencode 2.x the plugin's live state comes from its server entrypoint, which runs in the shared service and has to be listed in `opencode.jsonc` (ADR 0206 decision 23, amending ADR 0126). An operator who lists `dist/v2` only in `cli.json`, as plugin 0.2.0 asked, loads the TUI entrypoint alone: pane attribution works, but no session reports live state. Research note 03 (section 5.2, option A) describes a fallback: the TUI entrypoint sends snapshots for its routed session's family from its own stores (`ctx.data.session.status`, `session.permission.list`, `session.form.list`), accepting that sessions no client shows are missed. Two writers for one session would alternate in the store (different plugin instances), so the fallback must stay quiet while a server entrypoint reports. Found while building the OpenCode slice (CSP-816).
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [x] #1 With only cli.json listing the plugin, a 2.x client's routed session and its child sessions report live state
- [x] #2 With the server entrypoint loaded, the TUI entrypoint sends no live-state snapshots
- [x] #3 Event scripts in tests/fixtures/hooks/opencode-live cover the fallback, replayed by the plugin tests and the Rust tests
<!-- AC:END -->

## Definition of Done
<!-- DOD:BEGIN -->
- [x] #1 `just check` passes, or `git diff --check` for docs-only changes
- [x] #2 Docs and ADRs are updated when behavior, architecture, or workflow change
<!-- DOD:END -->

## Implementation Plan

<!-- SECTION:PLAN:BEGIN -->
1. Findings: the 2.0.32 TUI context has `client` (opencode's own client, whose `plugin.list({location})` returns the service's plugins with `id`, `features.server`, and `state.status`), and data stores `session.status(id)` ("idle"|"running"), `session.permission.list(id)`, `session.form.list(id)`, `session.family(id)`; `ctx.data.listen` delivers the service's whole event stream.
2. Pure fallback module (src/v2/fallback.ts): run the 2.x reducer over the client's stream; report only sessions the client has shown (the routed session, its family, their descendants), from first route until the client closes; fill a joining session in from the stores through the reducer's seed; report only while the service's plugin list shows no active server entrypoint with this plugin's id (quiet until it has answered, a failed check changes nothing, re-asked every 60 s); send nothing when the client closes. Reducer gains `resend` and `lineage`.
3. TUI entrypoint glue: keep attribution as is, feed the fallback, poll plugin.list.
4. Tests: two shared scripts (v2-tui-fallback, v2-tui-server-reports) replayed by the plugin tests and the Rust tests; a node test that loads tui.ts against a stand-in context and writer.
5. ADR 0229 (amends ADR 0126's no-server-API line for this one read-only call through opencode's client, replaces ADR 0206 decision 23's last sentence); README, operations guide, opencode_live.rs docs, CHANGELOG.
<!-- SECTION:PLAN:END -->

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
Detection choice: the TUI context exposes opencode's own client (`ctx.client`), whose `plugin.list({location})` answers with the service's plugins (`id`, `features.server`, `state.status`); the fallback reports only while no active server entrypoint with id conspectus-hook is listed, is quiet until the first answer, keeps its mode on a failed check, re-asks every 60 s. This is one read-only call to the service, which ADR 0126 had ruled out for the TUI entrypoint; ADR 0229 amends it (no credential handled by Conspectus). Other choices for review: sessions stay reported once shown (sticky until the client closes); snapshots carry the client's pid, so a session reads exited/killed once every client that showed it has closed; closing the client sends nothing.
Validation: just opencode-plugin-test 40/40 (scripts v2-tui-fallback and v2-tui-server-reports, tui.ts loaded under node against a stand-in context and writer, plugin-list field-read proxy test); Rust replays a_v2_client_*; checks.sh green; just check 5685 passed, 1 skipped. tsc not run (no npm install offline); tui.ts, fallback.ts, write.ts run under node's type stripping.
<!-- SECTION:NOTES:END -->

## Final Summary

<!-- SECTION:FINAL_SUMMARY:BEGIN -->
With the plugin listed only in `cli.json`, the opencode 2.x TUI entrypoint (plugin 0.4.0) now reports live state itself: it runs the 2.x reducer over the events its client receives and reports the sessions the client has shown (the routed session, its family from `ctx.data.session.family`, and their subagents) from the first route until the client closes, filling a joining session in from the client's stores (`session.status`, `session.permission.list`, `session.form.list`) through the reducer's seed. It reports only while the service's plugin list (`ctx.client.plugin.list`, opencode's own client, asked at load and every 60 s) shows no active server entrypoint with this plugin's id, and is quiet until that list answers, so the client and the service never both write a session (AC2). Snapshots carry the client's pid; closing the client sends nothing. Attribution is unchanged. New pure module `src/v2/fallback.ts`; reducer gains `resend`/`lineage`. ADR 0229 (amends 0049, 0126's "never calls the server API", 0206 decision 23); README, operations guide, fixture README, `opencode_live.rs` docs, CHANGELOG updated.

Verified with `just opencode-plugin-test` (40 pass: scripts `v2-tui-fallback` and `v2-tui-server-reports`, `tui.ts` loaded under node against a stand-in context and writer, field-read proxy test of the plugin list), their Rust replays, `/tmp/csp-fu/checks.sh`, and `just check` (5685 passed, 1 skipped). `tsc` was not run (no `npm install` offline).
<!-- SECTION:FINAL_SUMMARY:END -->
