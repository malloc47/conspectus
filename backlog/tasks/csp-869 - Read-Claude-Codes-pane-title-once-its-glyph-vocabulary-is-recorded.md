---
id: CSP-869
title: Read Claude Code's pane title once its glyph vocabulary is recorded
status: Done
assignee: []
created_date: '2026-11-13 04:56'
updated_date: '2026-11-16 23:26'
labels:
  - live-state
  - ls-policy
milestone: m-26
dependencies:
  - CSP-822
  - CSP-831
priority: low
ordinal: 949000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Claude Code prefixes its terminal title with a status glyph in terminals without OSC 7501 support, but which glyph means which state is unverified (research note 01 section 5.4). ADR 0217 built the pane-title source for Codex only and gave the harness adapter a `pane_title_mark` method that Claude Code leaves at its default (nothing read). Once a real Claude Code session's titles are recorded, its adapter can classify them the same way.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [x] #1 Claude Code's title glyphs for working, waiting, and idle are recorded from a real session, with the version they came from
- [ ] #2 The Claude Code adapter classifies its pane titles with a golden table of the recorded strings, and ADR 0217 is amended to name the harness and its marks
- [x] #3 The operator guide's pane-title section lists Claude Code and what its title can and cannot show
<!-- AC:END -->

## Definition of Done
<!-- DOD:BEGIN -->
- [x] #1 `just check` passes, or `git diff --check` for docs-only changes
- [x] #2 Docs and ADRs are updated when behavior, architecture, or workflow change
<!-- DOD:END -->

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
2026-11-16 (CSP-822 recording, Claude Code 2.1.322 in tmux 3.7c): the pane title carries no state. It reads `✳ Claude Code` and then `✳ <session name>`, and does not change while the session is busy, waiting, or idle. There is no glyph vocabulary to read, so this task has no work left unless a later version adds one.
<!-- SECTION:NOTES:END -->

## Final Summary

<!-- SECTION:FINAL_SUMMARY:BEGIN -->
Closed without building the source: Claude Code's pane title carries no state. CSP-822 recorded it from a real Claude Code 2.1.322 in tmux 3.7c: `✳ Claude Code`, then `✳ <session name>`, unchanged while busy, waiting, and idle, so there is no glyph vocabulary to read and nothing for a golden table or an ADR 0217 amendment to hold (acceptance criterion 2 is left unchecked for that reason). The recorded title and the version are in `docs/plans/live-state/research/01-claude-code.md`, and the operator guide's pane-title section now says what Claude Code's and Pi's titles cannot show. Verified with `git diff --check`; a docs-only change.
<!-- SECTION:FINAL_SUMMARY:END -->
