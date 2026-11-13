---
id: CSP-871
title: >-
  Show live state from claude agents --json for Claude Code sessions with no
  hook
status: To Do
assignee: []
created_date: '2026-11-13 05:22'
labels:
  - live-state
  - ls-policy
milestone: m-26
dependencies:
  - CSP-832
priority: low
ordinal: 951000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
ADR 0218 runs `claude agents --json` only as a repair of a Claude Code hook entry stuck at `working` or `waiting`, and gives a session with no hook entry nothing: the poll runs only while some hook entry is due, so a hook-less session's state would appear by accident and then never age (`ready` and `waiting` hold while the process lives, ADR 0203). A Claude Code session whose hooks are untrusted, disabled (`disableAllHooks`, `--bare`, managed hooks only), or not installed therefore shows no live state, and `hook status` says `silent`.

Showing those sessions from the poll needs a cadence of its own (a regular poll whenever a Claude Code process with no hook entry is known, at some cost per call), a rule for how a poll-only state ages once polls stop, and a binding by pid to the Claude Code process discovery already knows. It is an operator decision under ADR 0207 decision 4 and needs its own ADR.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 An ADR decides the cadence, the cost bound, how a poll-only state ages, and how it binds to a session or pane, and the operator approves it
- [ ] #2 A Claude Code session with no hook entry shows the poll's state with source harness_poll, and loses it when polls stop, by tests with a fake runner and clock
- [ ] #3 hook status reports how many Claude Code agents the poll shows, as it does for Codex pane titles
<!-- AC:END -->

## Definition of Done
<!-- DOD:BEGIN -->
- [ ] #1 `just check` passes, or `git diff --check` for docs-only changes
- [ ] #2 Docs and ADRs are updated when behavior, architecture, or workflow change
<!-- DOD:END -->
