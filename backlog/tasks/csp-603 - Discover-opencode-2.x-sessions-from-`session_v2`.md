---
id: CSP-603
title: Discover opencode 2.x sessions from `session_v2`
status: To Do
assignee: []
created_date: '2026-10-17 02:58'
updated_date: '2026-10-18 05:57'
labels:
  - opencode-v2
milestone: m-21
dependencies:
  - CSP-602
references:
  - 'https://github.com/Dicklesworthstone/cross_agent_session_resumer/issues/30'
priority: high
ordinal: 652000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
The opencode adapter (`src/discovery/harness/opencode.rs`) reads sessions from the 1.x `session` table and the last-message preview from `part`. OpenCode 2.x writes live sessions to `session_v2` and their messages to `session_message`, and leaves the 1.x tables in place but stale after migration. Against a 2.x database Conspectus would show only pre-upgrade sessions, with old recency, and every new session would be missing from the graph, the TUI, and pin binding. If migration copies 1.x sessions into `session_v2` under the same ids, reading both layouts naively would also duplicate them.

Related open work: `session_v2.parent_id` bears on subagent detection (CSP-295), and `fork_session_id` is new lineage evidence for the ADR 0018 lineage model.

- Scope: detect the 2.x layout and read `session_v2` (id, directory, title, parent lineage, recency) with the preview taken from `session_message`; keep 1.x and legacy `storage/` support; never emit the same session twice when both layouts are present; keep the opencode.db scan cache (CSP-518) effective; read only, with the existing query-only connection settings (ADR 0048).
- Tests: fixture tests against the CSP-602 fixtures for a fresh 2.x database, a migrated database, a 1.x database, and a database with no sessions.
- Manual checks: against a sandboxed 2.x install, a new session appears in `conspectus graph` and the TUI with the right cwd, title, and recency.
- Blockers: CSP-602.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 Sessions stored only in `session_v2` appear as agent-session nodes with id, cwd, title, and last-active time
- [ ] #2 A migrated database yields each session exactly once, preferring the `session_v2` row
- [ ] #3 The last-message preview for 2.x sessions comes from `session_message`
- [ ] #4 `session_v2.parent_id` produces the same parent-session lineage links that 1.x `parent_id` does
- [ ] #5 1.x-only databases and legacy `storage/` sessions keep their current behavior, with existing tests passing unchanged
<!-- AC:END -->

## Definition of Done
<!-- DOD:BEGIN -->
- [ ] #1 `just check` passes, or `git diff --check` for docs-only changes
- [ ] #2 Docs and ADRs are updated when behavior, architecture, or workflow change
<!-- DOD:END -->

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
Scope update from the CSP-602 spike (ADR 0125):
- A fresh 2.x database has no 1.x `session`/`message`/`part` tables at all, so discovery must not require the `session` table.
- Migration copies 1.x sessions into `session_v2` under the same ids; the 1.x rows stay, unmodified. Prefer the `session_v2` row on id collision.
- `session_v2.parent_id` is set only on sessions spawned by the `subagent` tool (child row carries `agent`, title = task description), so emit it as `spawn` lineage (ADR 0048 vocabulary) and classify the child as a subagent. Forks set `fork_session_id` instead (parent_id null), so emit `fork` lineage for it.
- Preview: the newest non-empty `user.text` or assistant `content[type=text].text` by `seq`.
- Fixtures: `tests/fixtures/opencode/v2-fresh.sql`, `tests/fixtures/opencode/v2-migrated.sql`.
<!-- SECTION:NOTES:END -->
