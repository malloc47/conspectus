---
id: CSP-613
title: Add a native Pi transcript parser
status: Done
assignee:
  - '@claude'
created_date: '2026-10-17 03:02'
updated_date: '2026-10-17 14:39'
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
- [x] #1 The parser renders the active branch (leaf to root) in order, and abandoned `/tree` branches are absent from the default view
- [x] #2 User, assistant text, tool calls, and tool results render; compaction and branch summaries render as summary markers
- [x] #3 `system` messages, `custom` entries, `custom_message` entries with `display: false`, and metadata entries are hidden by default and visible in the raw view
- [x] #4 A missing file maps to `ParseError::NotFound` and malformed lines degrade without failing the whole document
- [x] #5 Parser tests use fixtures with a branch, a compaction, a branch summary, and an extension message; a viewer snapshot covers a Pi session
<!-- AC:END -->

## Definition of Done
<!-- DOD:BEGIN -->
- [x] #1 `just check` passes, or `git diff --check` for docs-only changes
- [x] #2 Docs and ADRs are updated when behavior, architecture, or workflow change
<!-- DOD:END -->

## Implementation Plan

<!-- SECTION:PLAN:BEGIN -->
1. The viewer has no raw-view toggle (AC #3 assumed one). The operator chose a generic one: ADR 0122 adds `TranscriptTurn::hidden`, a `show_hidden` state field, `H` (ToggleHidden), a footer chip shown only when the transcript has hidden turns, a `?` help entry, and the filter in `passes_turn_filters`.
2. Add `src/viewer/parser/pi.rs`. It finds the file under the session root (by file name, then header id), walks `parentId` from the last entry to the root (v1 files stay linear), and maps entries to turns. Pi-hidden records become `hidden`.
3. Point the Pi adapter's `transcript_source` and `transcript_parser` at it.
4. Add a fixture with a `/tree` branch, a branch summary, a compaction, an extension state entry, `custom_message` display false and true, a system prompt, tools, thinking, metadata, and a malformed line. Add parser tests, widget tests for the toggle, and viewer snapshots.
<!-- SECTION:PLAN:END -->

## Final Summary

<!-- SECTION:FINAL_SUMMARY:BEGIN -->
Pi sessions now open in the native viewer and the inline transcript preview.
- `PiParser` renders the active branch by walking `parentId` from the leaf (the last entry), so abandoned `/tree` branches don't appear. v1 linear files render in order.
- User and assistant text, thinking, tool calls, and tool results render. Compaction and branch summaries render as `compact` markers.
- Records Pi's UI never shows become hidden turns: the system prompt, `custom` state, `custom_message` with `display: false`, model and thinking-level changes, labels, usage, context edits, and the session name.
- The viewer gained a generic `H` toggle for hidden turns (ADR 0122, the operator's choice, since no raw view existed). Its footer chip appears only when the transcript has hidden turns. In a narrow footer it is kept in place of the aborted chip when there are hidden turns and no aborted ones, so other harnesses' footers are unchanged.

Verification:
- `just check`: 2384 tests, clippy, fmt, rustdoc, and the diff check.
- 12 `viewer::parser::pi` tests cover:
  - the active branch order, with abandoned branches absent,
  - tools, thinking, and both summary kinds,
  - the six hidden records, and that a `custom_message` with `display: true` stays visible,
  - timestamps,
  - the v1 linear file,
  - flat layout, file-name lookup, and header-id lookup,
  - `NotFound`,
  - a dangling parent, and a parent cycle (which caught a leaf-duplication bug, now fixed).
- New reducer and widget tests cover the `H` toggle and its chip. New viewer snapshots `pi_session_renders_the_active_branch` and `pi_session_with_hidden_records_shown`.
- Manual: `tui --snapshot` against the fixture, with the cursor on the Pi row, shows the inline transcript preview of the active branch with hidden records filtered out. Snapshot mode can't open the full modal viewer.

Pi is not installed on this machine, so comparing against what `pi --session <id>` shows was not done.
<!-- SECTION:FINAL_SUMMARY:END -->
