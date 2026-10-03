---
id: CSP-349
title: Detect usage-limit tails in supported transcript parsers
status: To Do
assignee: []
created_date: '2026-06-03 12:44'
labels:
  - h-continue
milestone: m-11
dependencies:
  - CSP-348
ordinal: 227000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
- Scope: for Claude Code, Codex, and OpenCode, inspect the final
  meaningful assistant/system message after applying the same
  tool/thinking/channel-marker filters used by preview and viewer
  parsing. Recognize usage-limit messages only when they are the
  last meaningful transcript message and contain a parseable resume
  time. Preserve the source snippet and parsed time without
  fabricating a blocked state for older messages in the middle of a
  transcript.
- Tests: fixtures for absolute timestamps, relative "try again in"
  durations, timezone-bearing text, malformed/no-time messages,
  usage-limit messages followed by later user/assistant text, and
  ordinary transcript tails.
- Blockers: `CSP-348`.
<!-- SECTION:DESCRIPTION:END -->

Legacy ID: `H-CONTINUE-003`
