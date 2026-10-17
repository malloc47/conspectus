---
id: CSP-613
title: Add a native Pi transcript parser
status: To Do
assignee: []
created_date: '2026-10-17 03:02'
updated_date: '2026-10-17 03:05'
labels:
  - h-pi
milestone: m-22
dependencies:
  - CSP-610
references:
  - >-
    https://github.com/earendil-works/pi/blob/main/packages/coding-agent/docs/session-format.md
  - >-
    https://github.com/earendil-works/pi/blob/main/packages/coding-agent/docs/message-types.md
documentation:
  - docs/adr/0052-native-session-viewer.md
  - docs/adr/0053-viewer-abort-filter-default-harness-ux.md
priority: low
type: feature
ordinal: 662000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
The native transcript viewer (ADR 0052) opens claude-code, codex, and opencode sessions in place; Pi sessions would fall through to "no native viewer". This story adds a `HarnessParser` for Pi JSONL plus `transcript_source` and `transcript_parser` on the Pi adapter.

Pi's file is a tree, not a list. The conversation the operator sees in Pi is the active branch: walk `parentId` from the leaf (the last entry) to the root. Abandoned `/tree` branches remain in the file. Entry types that matter: `message` (roles `user`, `assistant` with text/thinking/toolCall blocks, `toolResult`, and `system`, which holds prompt sections and tool declarations), `compaction` and `branch_summary` (summaries), and `custom_message` (rendered only when `display` is true). `custom`, `label`, `usage`, `model_change`, `thinking_level_change`, and `context_edit` are metadata. Per the viewer-matches-harness rule (ADR 0053), what Pi hides stays hidden by default and the raw view goes behind the existing toggle.

Manual checks: open a branched, compacted Pi session in the TUI viewer and compare it with what `pi --session <id>` shows.

Blockers: the discovery adapter story.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 The parser renders the active branch (leaf to root) in order, and abandoned `/tree` branches are absent from the default view
- [ ] #2 User, assistant text, tool calls, and tool results render; compaction and branch summaries render as summary markers
- [ ] #3 `system` messages, `custom` entries, `custom_message` entries with `display: false`, and metadata entries are hidden by default and visible in the raw view
- [ ] #4 A missing file maps to `ParseError::NotFound` and malformed lines degrade without failing the whole document
- [ ] #5 Parser tests use fixtures with a branch, a compaction, a branch summary, and an extension message; a viewer snapshot covers a Pi session
<!-- AC:END -->

## Definition of Done
<!-- DOD:BEGIN -->
- [ ] #1 `just check` passes, or `git diff --check` for docs-only changes
- [ ] #2 Docs and ADRs are updated when behavior, architecture, or workflow change
<!-- DOD:END -->
