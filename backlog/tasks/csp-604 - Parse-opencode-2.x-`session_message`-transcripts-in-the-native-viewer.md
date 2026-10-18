---
id: CSP-604
title: Parse opencode 2.x `session_message` transcripts in the native viewer
status: To Do
assignee: []
created_date: '2026-10-17 02:58'
updated_date: '2026-10-18 05:57'
labels:
  - opencode-v2
milestone: m-21
dependencies:
  - CSP-602
  - CSP-603
priority: medium
ordinal: 653000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
The native viewer's opencode parser (`src/viewer/parser/opencode.rs`, CSP-337) joins the 1.x `message` and `part` tables. OpenCode 2.x stores a session's transcript as `session_message` rows ordered by `seq`, with a typed JSON `data` blob. Reported message types are user, assistant, system, synthetic, compaction, model-switched, agent-switched, shell, and location-switched. Assistant blobs carry `{agent, model, content: [...]}`, whose items are reasoning, text, or tool parts; tool parts use `id` rather than `callID` and keep their outcome in `state` (status, input, output, content, error). For 2.x sessions the viewer would show an empty or stale transcript.

Per the operator's viewer preference, the default rendering should match what opencode's own UI shows, with harness bookkeeping hidden unless the raw view is on.

- Scope: render 2.x transcripts from `session_message` with the same turn kinds the 1.x parser produces (messages, thinking, tool call and result, aborted turns); choose the parser by layout so 1.x sessions keep working; decide how the 2.x-only types (compaction, model/agent/location switches, shell) render.
- Tests: parser tests over the CSP-602 fixtures for each message type and tool state, plus an unknown-type row that must not fail the whole transcript.
- Manual checks: open a sandboxed 2.x session in the viewer and compare it with opencode's TUI.
- Blockers: CSP-602, CSP-603.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 `v` on a 2.x opencode session shows its user, assistant, reasoning, and tool turns in `seq` order
- [ ] #2 Tool turns show input and output from the 2.x `state` shape, including errored tools
- [ ] #3 Compaction and model, agent, or location switches render as compact markers or stay hidden by default, as decided in the task
- [ ] #4 Unknown message types are skipped without failing the transcript
- [ ] #5 1.x transcripts render as before
<!-- AC:END -->

## Definition of Done
<!-- DOD:BEGIN -->
- [ ] #1 `just check` passes, or `git diff --check` for docs-only changes
- [ ] #2 Docs and ADRs are updated when behavior, architecture, or workflow change
<!-- DOD:END -->

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
Scope update from the CSP-602 spike (ADR 0125):
- Verified shapes: `user` (`text`), `assistant` (`content` items `text` / `reasoning` / `tool`), `idle` (`outcome` succeeded / failed / interrupted), `system` (`text`, injected by migration). Tool items: `id`, `name`, `state.status` completed (`input`, `content[]` of `{type: text, text}`) or error (`input`, `error.message`).
- Schema-only shapes (from `@opencode/schema` 2.0.32, not reproduced): `synthetic`, `skill`, `shell`, `compaction` (running/completed/failed with `summary`), `model-switched`, `agent-switched`, `location-switched`.
- The 2.x abort signal is an `idle` row with outcome `interrupted`, not an assistant `MessageAbortedError`.
- Session cwd comes from `session_v2.directory`.
<!-- SECTION:NOTES:END -->
