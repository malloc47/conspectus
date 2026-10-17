---
id: CSP-602
title: 'Spike: capture opencode 2.x reference state and behavior'
status: To Do
assignee: []
created_date: '2026-10-17 02:58'
updated_date: '2026-10-17 03:03'
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
- [ ] #1 Findings are written down in an ADR under `docs/adr/` (or a dated doc it cites), covering storage, migration, CLI, process model, plugin context, and server API, each item marked verified or unverified
- [ ] #2 Sanitized fixture databases exist for a fresh 2.x install and for a 1.x database migrated to 2.x, small enough to commit as test fixtures
- [ ] #3 The opencode version(s) inspected are recorded
- [ ] #4 Dependent opencode-v2 tasks are updated wherever a finding changes their scope
<!-- AC:END -->

## Definition of Done
<!-- DOD:BEGIN -->
- [ ] #1 `just check` passes, or `git diff --check` for docs-only changes
- [ ] #2 Docs and ADRs are updated when behavior, architecture, or workflow change
<!-- DOD:END -->
