---
id: CSP-822
title: Record real payloads and verify live state against each harness
status: To Do
assignee: []
created_date: '2026-11-12 01:47'
updated_date: '2026-11-12 12:09'
labels:
  - live-state
  - ls-docs
  - needs-operator
milestone: m-26
dependencies:
  - CSP-811
  - CSP-815
  - CSP-816
  - CSP-817
documentation:
  - docs/plans/live-state/research/01-claude-code.md
  - docs/plans/live-state/research/02-codex.md
  - docs/plans/live-state/research/03-opencode.md
  - docs/plans/live-state/research/04-pi.md
priority: medium
type: task
ordinal: 908000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Phase 1 fixtures are synthetic, built from documentation, and every research note lists claims it could not verify: whether `PermissionRequest` fires for questions, what `SessionEnd` does on SIGHUP, the order of compaction events, whether Codex hooks run silently until trusted, whether OpenCode 2.x emits `session.status`, how long Pi's `session_shutdown` takes on `kill-pane`. Until a real session confirms them, the reducers encode guesses.

Scope: This story needs a real session of each harness, so it is the operator's call when to run it (it uses their accounts). Build a redacting recorder (every non-enum string replaced by a placeholder) that runs a harness in a throwaway project and private tmux server with a settings override, so the operator's own settings are never touched. Run the manual checks listed in each note, record the payload shapes as fixtures, replace the synthetic fixtures, fix any reducer that a real sequence contradicts, and update each note's UNVERIFIED appendix.

TUI changes: none.

Tests and checks: Each note's manual-check list is answered in the final summary; the sequence tests run against recorded payloads.

Blockers: The four harness stories, and the operator running real sessions.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 Every manual check in the four harness notes is answered or explicitly deferred
- [ ] #2 Recorded, redacted payloads replace the synthetic fixtures where they differ
- [ ] #3 Reducers match the recorded sequences
<!-- AC:END -->

## Definition of Done
<!-- DOD:BEGIN -->
- [ ] #1 `just check` passes, or `git diff --check` for docs-only changes
- [ ] #2 Docs and ADRs are updated when behavior, architecture, or workflow change
<!-- DOD:END -->

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
From CSP-815: claims the Codex slice encodes that a recording should confirm. Each is listed in `tests/fixtures/hooks/codex/README.md` and ADR 0206 decision 19.
- An approval, granted or declined, fires nothing until the next tool, `Stop`, or `Interrupt`.
- A subagent's `PostToolUse` carries its `agent_id`.
- Is `SubagentStop` per child or per child turn?
- `/clear` fires `SessionStart` (`clear`) with no `SessionEnd` for the old session.
- What does a plan-mode `Stop` mean? If a proposed plan is common, a `ready` detail may be worth an ADR 0203 amendment.
- Does `hook remove` renumbering groups really cost a `/hooks` re-review?
- Is the trust table really at `hooks.state.<key>`?
- Is the `/hooks` review wording `hook init codex` prints accurate for the recorded version?

From CSP-816 (OpenCode): the event scripts in `tests/fixtures/hooks/opencode-live/` are synthetic; their README lists the shapes to confirm. Also check in the sandbox: that 1.x `permission.asked` and `question.*` reach the plugin's `event` hook; that `dispose` runs when a 1.x TUI quits; that 2.x `ctx.event.subscribe` yields `{type, data, location: {directory}}` and whether a server plugin instance gets other locations' events (the reducer drops them); that the 2.x step and compaction events carry `data.sessionID`; the 2.x `execution.failed` error `type` words; and that a `dist/v2` entry in `opencode.jsonc` loads both entrypoints. The plugin's TypeScript was not compiled when it landed: run `npm install && npm run typecheck` in `plugins/opencode-hook` first.

From CSP-817 (Pi): the event scripts in `tests/fixtures/hooks/pi-live/` are synthetic; their README lists the event shapes to confirm. Also check in the sandbox: that Pi loads the extension (it was run under Node with a stand-in API, never compiled against Pi's types) and ignores its one extra export (`testOnly`); the field names of every handled event, including `after_provider_response.status` and `agent_settled.aborted`; whether a gate's dialog opens inside the tool's `tool_execution_start`/`_end` or before it; that `/reload` keeps the session id and calls the factory again; `ctx.isIdle()` at `session_start`; that a quit's and a `kill-pane`'s `session_shutdown` report lands; that the attribution record now carries Pi's own pid; and whether Pi exposes its version to an extension (CSP-851).
<!-- SECTION:NOTES:END -->
