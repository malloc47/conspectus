---
id: CSP-869
title: Read Claude Code's pane title once its glyph vocabulary is recorded
status: To Do
assignee: []
created_date: '2026-11-13 04:56'
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
- [ ] #1 Claude Code's title glyphs for working, waiting, and idle are recorded from a real session, with the version they came from
- [ ] #2 The Claude Code adapter classifies its pane titles with a golden table of the recorded strings, and ADR 0217 is amended to name the harness and its marks
- [ ] #3 The operator guide's pane-title section lists Claude Code and what its title can and cannot show
<!-- AC:END -->

## Definition of Done
<!-- DOD:BEGIN -->
- [ ] #1 `just check` passes, or `git diff --check` for docs-only changes
- [ ] #2 Docs and ADRs are updated when behavior, architecture, or workflow change
<!-- DOD:END -->
