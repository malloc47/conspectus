---
id: CSP-628
title: Show Codex system and developer messages behind the viewer hidden toggle
status: Done
assignee:
  - '@claude'
created_date: '2026-10-17 18:11'
updated_date: '2026-10-17 18:14'
labels:
  - h-ui
milestone: m-11
dependencies: []
references:
  - docs/adr/0122-viewer-harness-hidden-records-toggle.md
priority: low
type: enhancement
ordinal: 686000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
The Codex viewer parser drops `developer` and `system` role messages outright, so the operator can never see the instructions a Codex session ran under. ADR 0122 added a harness-neutral `hidden` flag on transcript turns with an `H` viewer toggle for exactly this kind of content: hidden by default to match Codex's own UI, but readable on demand. It names moving Codex's `system` and `developer` messages onto the flag as a follow-up. The `<turn_aborted>` / `<proposed_plan>` channel markers are noise, not content, and stay dropped.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [x] #1 Codex developer and system messages render as hidden system turns: absent by default, visible after the H toggle
- [x] #2 The H footer chip appears for Codex transcripts that contain such messages
- [x] #3 Channel markers stay dropped and user and assistant rendering is unchanged
- [x] #4 Parser tests cover developer and system messages and the codex parser module docs describe the mapping
<!-- AC:END -->

## Definition of Done
<!-- DOD:BEGIN -->
- [x] #1 `just check` passes, or `git diff --check` for docs-only changes
- [x] #2 Docs and ADRs are updated when behavior, architecture, or workflow change
<!-- DOD:END -->

## Implementation Plan

<!-- SECTION:PLAN:BEGIN -->
1. In emit_message_turns, map developer and system roles to TurnRole::System with hidden: true instead of returning early. 2. Keep channel-marker filtering for every role. 3. Update the module docs and replace the skip test with hidden-turn assertions plus a widget check that H reveals them.
<!-- SECTION:PLAN:END -->

## Final Summary

<!-- SECTION:FINAL_SUMMARY:BEGIN -->
The Codex viewer parser now emits `developer` and `system` messages as hidden `System` message turns instead of dropping them. They are absent by default and revealed by the `H` toggle (ADR 0122). The footer's `H` chip therefore appears for Codex transcripts that contain such messages. Channel markers (`<turn_aborted>`, `<proposed_plan>`) are still dropped in every role, and user and assistant rendering is unchanged.

Docs updated: the codex parser module docs, ADR 0122 decision 3 (which now records Codex as using the flag), and the `TranscriptTurn::hidden` doc comment, which is now harness-neutral.

Verification: `just check` passes (2385 tests). `developer_and_system_messages_are_hidden_system_turns` checks role, hidden flag, and order. `channel_markers_are_dropped_in_every_role` is new. The generic widget test `hidden_turns_are_filtered_until_toggled` now uses a Codex-shaped document and checks the chip and that `H` reveals the developer turn.
<!-- SECTION:FINAL_SUMMARY:END -->
