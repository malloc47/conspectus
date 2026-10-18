---
id: CSP-602
title: 'Spike: capture opencode 2.x reference state and behavior'
status: Done
assignee:
  - '@claude'
created_date: '2026-10-17 02:58'
updated_date: '2026-10-18 05:58'
labels:
  - opencode-v2
milestone: m-21
dependencies: []
references:
  - 'https://anoma.ly/notes/opencode-2-0/'
  - 'https://opencode.ai/v2/docs/migrate-v1/'
  - 'https://github.com/Dicklesworthstone/cross_agent_session_resumer/issues/30'
priority: high
ordinal: 651000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
OpenCode 2.0 (announced at https://anoma.ly/notes/opencode-2-0/) changes the storage schema, the plugin API, the server API, and the process model: one background service now backs every TUI, web and desktop client. Conspectus reads opencode state, launches and resumes it in tmux, and attributes sessions to panes through an in-process plugin, so all of that needs re-checking against real 2.x behavior. Nothing in the announcement or the V1 migration guide documents on-disk storage, CLI flags, or the service's process shape.

What is known so far, from third-party reports rather than verified against a live install:
- 2.x keeps `~/.local/share/opencode/opencode.db` but stores live sessions in new `session_v2` (id, project_id, workspace_id, parent_id, fork_session_id, slug, directory, title, version, model JSON, cost, tokens_*, time_created, time_updated) and `session_message` (id, session_id, type, seq, time_created, time_updated, data JSON, ordered by `seq`) tables. The 1.x `session`/`message`/`part` tables remain, stale, after migration (see the casr issue in the references).
- Local opencode 1.18.32 already has an empty `session_message` table and no `session_v2`.
- Plugins run inside the shared service, not inside each TUI process; 1.x plugins do not load in 2.0.

The machine this was scoped on runs opencode 1.18.32. Do not upgrade the operator's install: capture 2.x in a sandbox (separate `HOME`/`XDG_*` dirs or a nix-built 2.x), including a migrated copy of a 1.x database.

- Scope: record findings for the dependent `opencode-v2` tasks: schema of both tables and of `session_message.data` per message type; whether migration copies 1.x sessions into `session_v2` under the same ids; TUI launch and session resume CLI (`opencode --session <id>` or its replacement); the background service's process name, argv, parent, lifetime, and inherited environment (`TMUX`, `TMUX_PANE`); what a TUI client looks like as a tmux pane process and which files it holds open; plugin execution context and the event names and payloads available; whether the V2 server API exposes a read-only mapping from connected clients or tabs to session ids.
- Tests: none; this task produces fixtures and findings.
- Manual checks: confirm no step touches the operator's real opencode state.
- Blockers: none.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [x] #1 Findings are written down in an ADR under `docs/adr/` (or a dated doc it cites), covering storage, migration, CLI, process model, plugin context, and server API, each item marked verified or unverified
- [x] #2 Sanitized fixture databases exist for a fresh 2.x install and for a 1.x database migrated to 2.x, small enough to commit as test fixtures
- [x] #3 The opencode version(s) inspected are recorded
- [x] #4 Dependent opencode-v2 tasks are updated wherever a finding changes their scope
<!-- AC:END -->

## Definition of Done
<!-- DOD:BEGIN -->
- [x] #1 `just check` passes, or `git diff --check` for docs-only changes
- [x] #2 Docs and ADRs are updated when behavior, architecture, or workflow change
<!-- DOD:END -->

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
Sandbox: opencode 2.0.32 from the npm `@opencode/cli-linux-x64` tarball (integrity checked, interpreter patched for NixOS) and the operator's 1.18.32 nix build, each run under scratch HOME/XDG_* dirs against a local mock OpenAI-compatible model. tmux probes used a private tmux server. A probe plugin with server and tui entrypoints logged pid, TMUX_PANE, router state, and events from both sides.
Operator state check: the real opencode.db still has no session_v2 table or 2.x migrations, and ~/.config/opencode has no service.json.
Unverified (recorded in the ADR): model-switch and /compact runs hung in the sandbox, so those message shapes come from @opencode/schema only; the 1.x `task` subagent run hung as well.
<!-- SECTION:NOTES:END -->

## Final Summary

<!-- SECTION:FINAL_SUMMARY:BEGIN -->
Captured opencode 2.x reference behavior in a sandbox and recorded it in ADR 0125, with each finding marked verified or unverified: storage (session_v2, session_message shapes), migration (same-id copy, 1.x tables left stale), CLI (`--session` unchanged, creates unknown ids), process model (shared `opencode serve --service` inherits the first client's TMUX; TUIs hold no DB fds), plugins (tui entrypoint runs in the pane with the client's current session), and server API (no client-to-session mapping). Added sanitized SQL fixtures for a fresh 2.x and a migrated 1.x database, and appended scope updates to CSP-603..608.
<!-- SECTION:FINAL_SUMMARY:END -->
